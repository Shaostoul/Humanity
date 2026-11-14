# Skills: the base set, how a skill levels, what a level gives

**Status: a proposal for the operator to decide, 2026-11-14. Nothing here is a decision** except what
is quoted from him. The skill table in code is still today's twenty rows (one renamed: alchemy is now
chemistry, his decision); no new skill enters `data/skills/skills.csv` until he approves the set.
The evidence is [docs/reference/research/2026-11-14-skill-systems/](../reference/research/2026-11-14-skill-systems/README.md)
(eight real-world frameworks and ten games, each quoted and dated).

Why this exists, in his words (2026-11-14, [activities.md](activities.md) section 6):

- "Let's not focus on a cap. Let's make sure we make a proper base set of skills before we go
  throwing numerical limits down."
- "We need to figure out what skills we should bother including. Then we need to make sure we
  understand how they are leveled and what leveling them does for us."
- "We want offense and defense. We want to include everything as to achieve the claim of HumanityOS
  eventually being able to teach everyone everything."
- "why would fishing make me better at finding mushrooms?"
- "Maybe like prop manipulation or be like staff, whip, etc. since learning to use a whip is way
  different from a staff."

Companions: [activities.md](activities.md) (the 93 activities; `data/gameplay/activities.json`),
[progression-skills-gear.md](progression-skills-gear.md) (skills, abilities and gear as one stat
pipeline; its 20-skill rule is retired), [educational-gameplay.md](educational-gameplay.md) (the
Novice, Competent, Expert, Master table and "teach everything"),
[engagement-modes.md](engagement-modes.md) (nobody is forced through an activity),
[two-realities.md](two-realities.md) (every domain exists twice).

## 0. In plain words

- A **skill** is something you get better at by doing it the real way: baking, wiring, poi. The
  proposal has **90 skills in 11 families**. 25 could rise as soon as existing XP is pointed at them,
  28 have an activity that runs but gives no XP yet, and 37 wait for their activity to be built.
- **A skill splits when the movement or the knowledge is genuinely different**, the operator's whip
  and staff test, and only once the game can teach the difference. So staff and whip are two skills,
  fishing and foraging are two skills, and "engineering" becomes electrical, plumbing, mechanics and
  the rest.
- **You level a skill by doing the real procedure.** Work a little beyond you teaches most, a good
  result teaches more than a careless one, reading the right guide makes practice count for more, and
  practising beside a teacher helps. Repeating the same easy thing teaches less and less.
- **Levels are named stages, Novice to Master**, each a short list of "I can" statements. You move up
  a stage by showing you can do it: in the simplified mode your normal work is the proof; in the full
  realism mode it is a real assessment (do the task, answer why, and at higher stages a portfolio and
  someone two stages above to sign you off).
- **A stage gives** quicker hands, better results, fewer mistakes, fewer accidents, new recipes and
  tools, less hand-holding, and the right to teach and certify others. It is shown in words ("your
  loaves come out Good more often; shaping takes 2 minutes, was 4"), never a bare number.
- **Nobody has to grind.** Every need can be met another way (trade, automation, a cooperative, the
  background), the starter tier of every skill is open, and a person who already knows a skill can
  take the stage's demonstration at any time and pass it.
- **The real-life twin** keeps the same skills and stages for your real life: a learning log, "I can"
  statements with evidence, attestations signed by the people who watched you, and real certificates
  with their expiry dates.

## 1. What a skill is, and the rules for the set

**A skill is a body of doing, not knowing.** O*NET keeps skills and knowledge apart (research A1):
knowing soil chemistry is not growing a crop. So the game has three layers, each in its own place:

| Layer | What it is | Where it lives |
|---|---|---|
| Knowledge | What you understand: maths, physics, biology, the "why" of every craft | The curriculum, [`data/curriculum/syllabus.json`](../../data/curriculum/syllabus.json) (143 topics, each already listing the skills it serves), checked by questions |
| Skill | What you can do, earned by doing it | `data/skills/skills.csv`, one row per skill |
| Ability | An action you trigger, with a cost and a cooldown | `data/abilities.csv`, gated by a skill stage ([progression-skills-gear.md](progression-skills-gear.md) Part 2) |

Gear extends a skill and never replaces it (a good saw cuts straighter; it does not make you a joiner).

**The rules a skill must pass to be in the set:**

1. **It is real and assessable.** A real person learns it, and someone could watch them and say
   whether they can do it. Fantasy schools (pyromancy and the rest, today named by 94 rows of
   `data/abilities.csv` but rows of nothing) are not skills; section 9 asks what becomes of them.
2. **It splits where the movement or the knowledge is genuinely different** (the operator's whip and
   staff test): being good at one does not make you good at the other. Where the difference is only
   style (boxing and karate both strike; guitar and ukulele are both plucked strings) the styles are
   tags on one skill until the game can teach the difference.
3. **It splits only when the game can tell the difference.** A skill gets its own row when an
   activity teaches its distinct movement or knowledge. Dance stays one skill until the dance activity
   teaches two styles; poi and staff split now because the fire props design builds both.
4. **No skill without a trainer.** A row enters skills.csv in the same increment as the first code
   that awards it XP (the mirror of progression-skills-gear.md's rule that a new verb wires its XP).
   Today eight of the twenty rows can never rise; that never happens again.
5. **Transfers are declared, never assumed.** Related skills can give each other a head start where
   the overlap is real: staff spinning and a fighting staff, poi and rope dart, baton twirling and
   rifle drill. Fishing and foraging declare none, because finding mushrooms owes nothing to fishing.
   A transfer speeds learning of the other skill; it never grants its stages.
6. **One activity may train several skills** (a fire staff trains staff spinning and fire
   performance safety; aquaponics trains farming and keeping animals). Today the catalog has one
   `skill` field per activity; the proposal makes it a list.

## 2. The base set

Families are for reading and finding; a family has no level of its own. "Trains from" names the
activities in `data/gameplay/activities.json`. "Can rise": **today** means an XP call reaches it now
once existing rows or ids are pointed at it; **wire** means its activity runs but awards nothing yet;
**build** means its activity is designed or an idea. "Origin" says where the row comes from among the
current twenty. The coarse "can rise" column is this pass's reading and is rechecked against the code
when each row is added.

### Body and health

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `fitness` Fitness | Strength, endurance and mobility training | `fitness`, `sports` | build | new |
| `swimming` Swimming | Strokes, treading water, water safety and rescuing yourself | `swimming` | wire | new |
| `climbing` Climbing | Ladders, rock and rope climbing, and falling and landing without injury | `walking_climbing` | wire | new |
| `first_aid` First aid | Bleeding, burns, fractures, CPR, cold and heat injury, getting help | `first_aid_medicine`, `rescue_revival` | today | split from medicine |
| `medicine` Medicine | Diagnosing and treating illness and injury beyond first aid; medicines and doses | `first_aid_medicine`, `rescue_revival` | today | kept, narrowed |
| `sanitation` Hygiene and sanitation | Washing, toilets and waste, disinfection, stopping disease from spreading | `hygiene_sanitation` | wire | new |

### Food and water

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `farming` Gardening and farming | Soil, planting, watering, crop care, pests, harvest, compost, seed saving | `gardening_farming`, `composting`, `seed_saving`, `ecology`, `aquaponics`, `testing_lab` | today | kept, narrowed |
| `animal_husbandry` Keeping animals | Feeding, housing, health, breeding, milking, shearing, humane slaughter; fish in tanks | `livestock`, `aquaponics` | today | split from farming |
| `beekeeping` Beekeeping | Hives, colony health, swarms, honey and wax | `beekeeping` | wire | split from farming |
| `mushroom_growing` Growing mushrooms | Substrate, spawn, clean technique, fruiting conditions | `mushrooms` | wire | split from farming |
| `foraging` Foraging | Finding and identifying wild plants and fungi, their lookalikes, harvesting without harm | `foraging`, `botany_identification` | today | kept |
| `fishing` Fishing | Line, net and trap; reading water; cleaning fish | `fishing` | build | new |
| `hunting` Hunting and trapping | Finding game, shot placement, traps, field dressing, the law and the ethics | `hunting_butchering` | today | new |
| `butchery` Butchering | Breaking a carcass into cuts, hygiene, using the whole animal | `hunting_butchering`, `livestock` | build | new |
| `cooking` Cooking | Heat, technique, recipes and food safety | `cooking` | today | kept, narrowed |
| `baking` Milling and baking | Grain to flour, dough, proving, ovens | `baking_milling` | today | split from cooking |
| `preserving` Preserving food | Drying, salting, smoking, canning, cold storage, and what makes each one safe | `preserving` | today | split from cooking |
| `fermenting` Fermenting and brewing | Sauerkraut, yoghurt, vinegar, kombucha, beer and wine | `fermenting`, `brewing` | build | split from cooking |
| `nutrition` Nutrition | What food does in the body; planning meals that meet a person's needs | `eating_drinking`, `cooking` | build | new |
| `water_purification` Safe water | Finding, testing, purifying and storing drinking water | `water_safe`, `testing_lab` | wire | kept |

### Materials and making

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `forestry` Forestry and woodcutting | Felling, bucking, splitting and seasoning wood; tree care | `gathering_by_hand` | wire | new |
| `mining` Mining and quarrying | Breaking and moving rock and ore safely, by hand or by drone | `gathering_by_hand`, `drone_mining` | today | kept |
| `metallurgy` Smelting and metallurgy | Ore to metal, refining, alloys, slag | `smelting_refining`, `recycling_salvage` | today | split from metalworking |
| `smithing` Smithing | Forging with heat and hammer, heat treatment, making tools | `smithing` | today | renamed metalworking |
| `welding` Welding | Joining metal: arc, gas, brazing; reading a weld | `welding` | build | split from metalworking |
| `machining` Machining and fabrication | Lathe, mill, measuring to tolerance, 3D printing | `fabrication_printing` | build | split from engineering |
| `woodworking` Woodworking | Joinery, carving, furniture, wooden tools | `woodworking` | today | kept |
| `textiles` Spinning and weaving | Fibre to yarn to cloth; dyeing | `tailoring_weaving` | today | split from tailoring |
| `tailoring` Sewing and tailoring | Patterns, sewing, mending, garments | `tailoring_weaving` | today | kept, narrowed |
| `leatherworking` Tanning and leatherworking | Hides to leather; cutting, stitching, care | none yet | build | new (later row: no activity yet) |
| `ceramics` Pottery and ceramics | Clay, forming, glazing, firing; bricks, tiles, ceramic filters | `pottery` | today | new (the kiln's recipes) |
| `glassmaking` Glassmaking | Melting, blowing, casting and annealing glass | `glassmaking` | today | split from metalworking |
| `chemistry` Chemistry | Soap, medicines, fuels and other compounds; testing what is in a sample | `chemistry`, `testing_lab` | today | renamed alchemy (done 2026-11-14) |
| `electronics` Electronics | Components, soldering, circuits, finding faults on a board | `electronics` | today | split from engineering |

### Building and engineering

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `construction` Construction | Foundations, framing, walls, roofs; building to carry its load | `construction`, `ship_building` | today | renamed shelter_building |
| `masonry` Masonry | Brick, stone, concrete, mortar and plaster | `construction` | today | split from shelter_building |
| `drafting` Drafting and design | Measured drawings, plans, CAD, laying out a space | `home_layout`, `vehicle_building`, `ship_building`, `fabrication_printing` | wire | new |
| `electrical` Electrical | Circuits, loads, protection, solar and batteries, working safely | `electrical` | wire | split from engineering |
| `plumbing` Plumbing | Pipes, fittings, pumps, drainage, pressure | `plumbing` | wire | split from engineering |
| `hvac` Heating, cooling and ventilation | Heat flow, insulation, heat pumps, airflow | `climate_control` | wire | split from engineering |
| `life_support` Life support | Air, carbon dioxide removal, oxygen and water recovery aboard | `life_support`, `testing_lab` | wire | split from engineering |
| `networking` Networks and computers | Cabling, setting up networks, keeping systems running | `data_networking` | wire | split from engineering |
| `rigging` Rigging and ropes | Knots, pulleys, hoists, lifting and securing loads | `pulleys_rigging`, `hauling_logistics` | wire | split from engineering |
| `mechanics` Mechanics | Engines, drivetrains, suspension; building and fixing vehicles | `vehicle_building`, `ship_building`, `maintenance_repair` | today | split from engineering |
| `repair` Maintenance and repair | Finding faults, preventive care, fixing worn and broken things, salvage | `maintenance_repair`, `recycling_salvage` | wire | split from engineering |
| `logistics` Logistics | Storing, tracking, packing and moving goods | `hauling_logistics`, `storage` | wire | new |

### Moving and travel

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `driving` Driving | Ground vehicles: control, road sense, recovery | `driving` | wire | new |
| `flying` Flying aircraft | Atmospheric flight: takeoff, landing, weather | `shuttle_trip` | build | new |
| `spaceflight` Spaceflight | Orbits, burns, docking, station-keeping | `piloting`, `docking`, `shuttle_trip` | build | new |
| `eva` Spacewalking | Zero-g movement, suits, tethers, working outside | `zero_g_eva` | build | new |
| `drone_operation` Flying drones | Remote piloting, with the delay distance brings | `drone_piloting` | build | new |
| `navigation` Navigation | Maps, compass, stars, dead reckoning, route planning | `navigation`, `exploring` | wire | kept |

### Science

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `geology` Geology and prospecting | Rocks, minerals, reading the ground, finding ore | `geology`, `prospecting` | build | new |
| `astronomy` Astronomy | The sky, telescopes, the motions that navigation and calendars rest on | `astronomy` | wire | new |
| `weather` Reading the weather | Clouds, pressure, wind, forecasting | `weather_reading` | wire | new |

### Safety and rescue

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `fire_making` Fire making | Lighting, tending and putting out a fire; charcoal | `fire_making` | wire | kept |
| `firefighting` Firefighting and damage control | Putting out fires, sealing breaches, shoring, flooding | `firefighting`, `damage_control`, `disaster_response` | build | new |
| `radiation_safety` Radiation safety | Dose, shielding, contamination, solar storms | `radiation_shielding` | build | new |
| `rescue` Search and rescue | Finding, freeing and moving people safely | `rescue_revival`, `disaster_response` | build | new |
| `tracking` Tracking and wildlife | Reading sign, animal behaviour, staying safe around wildlife | `wildlife_safety`, `hunting_butchering` | wire | new |

### Combat (offense and defense)

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `unarmed` Unarmed striking | Punches, kicks, footwork and guards (boxing, kickboxing, karate as styles) | `combat` | wire | split from melee |
| `grappling` Grappling | Throws, holds, breakfalls and escapes (judo, wrestling, jiu-jitsu as styles) | `combat` | build | split from melee |
| `blades` Blades | Knife and sword: cuts, thrusts and parries (fencing, historical swordsmanship) | `combat` | today | split from melee |
| `impact_weapons` Clubs, axes and hammers | Weighted weapons swung by the handle | `combat` | today | split from melee |
| `polearms` Staff and spear | Long weapons held in two hands: staff, spear, bayonet | `combat` | today | split from melee |
| `archery` Archery | Bows and crossbows | `combat`, `hunting_butchering` | build | split from ranged |
| `marksmanship` Marksmanship | Rifles and pistols: safe handling, breathing, trigger control | `combat`, `hunting_butchering` | build | split from ranged |
| `throwing` Throwing | Spears, knives, axes and slings | `combat`, `hunting_butchering` | build | split from ranged |
| `gunnery` Gunnery | Turrets and mounted weapons: fire control, leading a target | `point_defense`, `combat` | build | new |
| `self_defence` Self-defence | Awareness, de-escalation, evading, blocking and escaping; wearing armour well | `combat` | build | renamed defense |
| `tactics` Tactics | Planning and coordinating a team in a fight or an emergency | `combat`, `point_defense` | build | kept |

### Community

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `trading` Trading | Prices, negotiation, bookkeeping | `trading` | wire | kept |
| `civics` Civics and governance | Proposals, voting methods, meetings, rules and rights | `governance`, `guilds_cooperatives` | wire | split from leadership |
| `leadership` Leadership | Leading a crew, delegating, planning, organising events | `leadership_crew`, `guilds_cooperatives`, `events_gatherings` | build | kept, narrowed |
| `teaching` Teaching | Explaining, demonstrating, checking understanding | `teaching` | build | new |
| `mediation` Conflict resolution | Listening, de-escalation, mediating disputes | `governance` | wire | new |

### Flow and prop arts

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `staff_spinning` Staff | Single, double and contact staff | `fire_props` | build | new |
| `poi` Poi | Poi: planes, timing, wraps | `fire_props` | build | new |
| `rope_dart` Rope dart and meteor | A weight on a long rope, wrapped and released around the body | `fire_props` | build | new |
| `hoop` Hoop | On-body and off-body hooping | `fire_props` | build | new |
| `fans` Fans | Fan spinning and isolations | `fire_props` | build | new |
| `twirling` Twirling and drill | Baton twirling, rifle and sabre drill spinning, the fire axe and fire knife | `fire_props` | build | new |
| `fire_performance` Fire performance safety | Fuels, wicks, dipping, spinning off, the safety spotter | `fire_props` | build | new |
| `juggling` Juggling | Toss juggling: balls, clubs, rings | none yet | build | new (later row: no activity yet) |
| `whip` Whip | Whip cracking and flow | none yet | build | new (later row: no activity yet) |

### Arts

| Skill | What it covers | Trains from | Can rise | Origin |
|---|---|---|---|---|
| `music` Music making | Rhythm, melody, harmony, arranging and composing | `music` | wire | new |
| `dance` Dance | Dance, split by style once the game teaches more than one | `dance` | build | new |
| `drawing_painting` Drawing and painting | Line, form, colour, composition | `painting` | build | new |
| `photography` Photography and film | Light, framing, exposure, editing | `photography_film` | wire | new |
| `writing` Writing | Clear, structured writing for any purpose | `writing` | build | new |

### Activities that train no skill, and why


| Activity | Why it trains no skill |
|---|---|
| `sleeping` | Rest is a need, not a trade. |
| `staying_warm` | A need met by other skills: fire making, tailoring (insulation) and first aid (cold and heat injury) each train when used for it. |
| `rail_transit` | Riding transit aboard is travel, not a trade. |
| `fleet_contribution` | An engagement mode: it gives away what other activities made, and those train their own skills. |
| `conversation` | Talking is living. It will train languages once people aboard speak other languages, and conflict resolution when a talk settles a dispute. |
| `running_machines` | No skill of its own: running a machine by hand trains the skill of what it makes (the smelter metallurgy, the oven baking); a machine left to run alone makes standard goods and trains nobody. |
| `watching_together` | Leisure. |
| `screen_games` | Leisure; a game with a real rating of its own (chess) can carry it later. |


**Later rows**, named so they are not forgotten, each waiting for an activity: languages (one row per
language, once people aboard speak them), sign language, programming (once in-world computers run
code), sailing and boating (the planet's oceans), horse riding, childcare and caregiving, and
instrument families for music (keyboard, plucked strings, bowed strings, winds, brass, percussion,
voice) once instruments are played by hand in the world. Leatherworking, juggling and whip are in the
tables above as later rows because the operator or a decision has already named them.

## 3. How a skill levels

### Stages, not numbers

The visible measure is a **stage**, with the same six names for every skill (Dreyfus, research A4,
extended by educational-gameplay.md's Master), each written for that skill as a few "I can"
statements (Scouts, A5):

| Stage | Plain meaning | Example: baking |
|---|---|---|
| 0 Untrained | Never practised | |
| 1 Novice | Follows the steps with the guide on; mistakes are common | I can bake a flatbread following the steps |
| 2 Beginner | Does the starter procedures alone and spots the common problems | I can bake a basic loaf and tell under-proved from over-proved |
| 3 Competent | Reliable results on routine work; plans their own work | I can bake a sandwich loaf, rolls and a sourdough without the guide |
| 4 Proficient | Handles the unusual and improvises | I can rescue a slack dough and adapt a recipe to the flour I have |
| 5 Expert | Fast, excellent work with no prompts | I can run a full bake for forty people to a standard |
| 6 Master | Creates new methods and certifies others | I can write a lesson and assess other bakers |

Every lower statement stays required. Inside a stage a progress bar fills with practice; today's XP
and numeric levels can keep running underneath (recipe gates are numeric levels, `skill_level` in
`data/recipes.csv`), with each stage a band of levels and a demonstration at each band's top. The
operator said not to set numerical limits yet, so this proposes no maximum: a skill has as many
levels as its stages need.

### What counts as practice

Practice is **doing the real procedure** (the activity's Direct mode), and XP for one attempt is
weighed by four things, so that grinding the cheapest action stops paying:

1. **Difficulty against your stage** (Wurm Online and Kenshi, B3 and B6). A task at or a little
   above your stage teaches most; one far below teaches almost nothing; one far above teaches a
   little, mostly from the failure. Each recipe or procedure already has a difficulty
   (`skill_level`).
2. **The quality of the result**, as the simulation grades it (`crates/humanity-systems/src/crafting/quality.rs`
   grades hand-made goods today). A careful good result teaches more than a careless one. A failure
   teaches something, once per kind of mistake.
3. **Variety.** The first time you make a thing is worth most; repeating the identical task loses
   value over an in-game day and recovers after.
4. **Help.** Reading the Library guide for your stage multiplies practice for a while and only at that
   stage (Project Zomboid's books, B5); practising beside someone two stages above multiplies it too
   (an apprenticeship).

Practice counts however you got to the bench: alone, in a cooperative crew, or taught. Watching does
not count; doing does.

### Knowledge checks

Each stage lists the curriculum topics it rests on. A short check (a few questions drawn from the
topic's Library guide, "why does low-acid food need a pressure canner?") is offered in the app. A
wrong answer shows the explanation, which is the point. Passing gives a one-time bonus and counts
toward the stage's assessment; a check never pays twice, so there is nothing to farm.

### Moving up a stage: the demonstration

A stage is earned by **showing** it (Starfield's challenges B9, Scouts A5, juggling A7), and the
evidence is a real competence check judged by the simulation, never a count:

- **Practical:** do the stage's set procedure to its standard (a weld that holds its load test, a loaf
  graded Good, a splint that holds the limb still, three-ball cascade for a set number of catches).
- **Knowledge:** pass the stage's check.
- **Portfolio**, from Competent up: a few things you made at this stage's standard (the farrier's
  board of shoes, A3).
- **A verifier**, from Proficient up: someone at least two stages above signs it (Scouts' "two down",
  A5), a player or an NPC mentor.

Every part must pass (the apprenticeship's weakest-link rule, A3); a failed part comes back with what
was wrong and how to fix it (the juggling federation's practice, A7), and can be retaken. **Anyone can
attempt any stage's demonstration at any time**: a person who already has the real skill shows it and
skips the practice bar (Scouts accept skills learned outside with evidence, A5). That is the strongest
guard against grinding and the fairest thing for people who already know.

### The two modes

CLAUDE.md: "Every deep system ships two modes", realistic severity exists and general play defaults
to softened. Proposed as one new `data/gameplay/levels.json` setting, `skill_learning`:

| | Off | Simplified (default) | Realistic |
|---|---|---|---|
| Stages | Every skill counts as Competent; nothing gates or grades | Earned by practice; the demonstration is your normal work (the last few results met the stage's standard) | Earned by the full demonstration above |
| Knowledge checks | Hidden | Offered, optional, a bonus | Part of each stage |
| Time in stage | None | None | A minimum of practice time at Expert and Master, which lengthens (judo, A6) |
| Out of practice | Never | Never | "Rusty" after a long gap: slower and more mistakes for a while, cleared by practice |
| Safety-critical certificates | None | None | First aid, electrical, firearms and the like carry a "current until" date a refresher renews (A8) |
| Accidents | None | As the activity's own setting says | Novices can hurt themselves on dangerous procedures |

**Decay, the plain answer: a stage is never lost.** Competence and currency are different things in
real life (A8): the stage is earned once; only "rusty" (realistic mode) and an expired certificate
(realistic mode) are temporary, and both clear by practice.

### Teaching others

Teaching is a skill of its own (`teaching`) and a way to advance: when you teach someone a skill you
are two stages above in, you earn teaching XP and a little in the skill itself, and from Proficient
you can verify learners' demonstrations. Masters certify (sign the assessment of an Expert) and write
lessons. This makes the Scouts' "two down" rule (A5) and judo's service route (A6) the game's
multiplayer teaching loop.

## 4. What a level gives

| It gives | What that means | Limit |
|---|---|---|
| Speed | The hands-on steps take less time | Never the natural processes: bread still proves for its hour, a crop still takes its season |
| Quality | Better grades more often (quality.rs's skill factor) | The materials and tools set a ceiling |
| Fewer mistakes | Steadier hands: aim, timing windows and measuring wobble shrink | **The level never makes your choices.** Choosing the temperature, the fix, the cut is the player's own knowledge; the level only steadies the hand that carries it out |
| Safety | Fewer burns, cuts and shocks on dangerous procedures (realistic mode) | Protective gear still matters |
| Unlocks | Recipes, tools and techniques at each stage, listed in the stage (RuneScape's visible ladder, B1); abilities through progression-skills-gear.md's `perk_levels` | The starter tier (today's `skill_level` 0 and 1) is always open |
| Less hand-holding | Step prompts fade by stage; an Expert sees none (Dreyfus, A4) | Anyone can turn the prompts back on |
| Trust | Verify learners from Proficient, certify from Master; crew roles that need a stage (the ship's electrician); better pay for skilled work (`data/economy.ron`'s `skill_scaling`) | |
| Recognition | The stage, its date and who verified it, on the profile | |

**How it shows.** A stage-up says, in words, what changed: "Baking: Competent. You can bake a
sandwich loaf without the guide. Shaping is quicker (about 2 minutes, was 4). Your loaves come out
Good more often." The Skills page shows each skill's stage, its "I can" list with what is done, what
the next stage asks, and what it will change. This follows the ELI5 mandate and the rule that every
setting says what it actually changes; every word comes from `data/i18n/en.json` by key (skill names
and descriptions move out of skills.csv into keys when the table is rebuilt).

**Engagement modes stay whole.** No skill is needed to meet a need: food, water, power and the rest
can be traded for, automated, pooled or left to the background (engagement-modes.md), so nobody grinds
a skill they dislike. Gates stay on what is genuinely dangerous or advanced. A machine left to run
alone makes standard goods and trains nobody, which is why hand work stays worth doing.

## 5. The real-life twin

The app's real-life side keeps the **same skill ids and stages**, so a person sees the game and their
life in one language, and never mixes them up:

- **A learning log.** Date, skill, what you practised, for how long, notes and, if you want, a photo
  or a video. Kept on your device and in your encrypted backup.
- **The "I can" list per stage**, self-assessed with evidence (a photo, a video, a witness), the way
  Scouts accept skills learned outside (A5).
- **Attestations.** Someone who watched you signs "I saw Sam do this on this date" with their own key.
  It is a client-held certificate, checked statelessly by anyone you show it to: the house pattern from
  the privacy arc, so no server holds a table of who taught whom (CLAUDE.md, the removed tables).
- **Real certificates.** First aid, a trade licence, a firearms course: who issued it, when, and when
  it expires, with a reminder before it does (a workplace first aid certificate lasts three years in Great Britain, A8). A photo of
  the certificate is optional; the app never asks for an official number.
- **The guides.** Each stage links its Library guides and curriculum topics.
- **Private by default.** You choose what to share and with whom (a guild looking for a certified
  electrician, a friend teaching you to bake).

**What exists today.** The web Tasks page lets a real-life task award XP to a linked skill
(`web/pages/tasks-app.js`, stored in the browser), and the relay keeps `reality_xp` apart from
`fantasy_xp` per skill. The proposal replaces "XP for real tasks" with the log, the evidence and the
attestations, because a points counter cannot measure a real skill and invites gaming it; the game
keeps XP. That is a question for the operator (section 9).

**Never merged.** A game stage is never a real certificate. The app can show both side by side when
the person chooses: "Baking, in the game: Competent. In real life: six bakes logged, verified by Ana."

## 6. Migration from today's twenty skills

| Today | Proposal |
|---|---|
| `foraging` | Kept. The walk-up collect's berries move here from farming |
| `shelter_building` | Renamed `construction`; `masonry` and `drafting` split out |
| `fire_making` | Kept; also trains the kiln's charcoal |
| `water_purification` | Kept (shown as "Safe water") |
| `navigation` | Kept |
| `woodworking` | Kept; `forestry` split out |
| `metalworking` | Renamed `smithing`; `metallurgy`, `welding` and `glassmaking` split out; its kiln recipes go to `ceramics` |
| `cooking` | Kept; `baking`, `preserving` and `fermenting` split out; `nutrition` new |
| `tailoring` | Kept as sewing; `textiles` split out; `leatherworking` a later row |
| `alchemy` | **Renamed `chemistry`, done in this change** (the operator's decision) |
| `melee` | Split: `unarmed`, `grappling`, `blades`, `impact_weapons`, `polearms` |
| `ranged` | Split: `archery`, `marksmanship`, `throwing`; `gunnery` new |
| `defense` | Renamed `self_defence`; each weapon skill keeps its own guards and parries |
| `tactics` | Kept |
| `trading` | Kept |
| `leadership` | Kept; `civics` split out; `mediation` new |
| `farming` | Kept; `animal_husbandry`, `beekeeping` and `mushroom_growing` split out |
| `mining` | Kept; `geology` new |
| `engineering` | Retired, split into `electrical`, `plumbing`, `hvac`, `life_support`, `networking`, `electronics`, `machining`, `mechanics`, `rigging` and `repair` |
| `medicine` | Kept; `first_aid` split out |

What the code change touches when the set is approved (none of it now):

- `data/skills/skills.csv`: the new rows, plus `family` and `transfers` columns; names and
  descriptions to i18n keys; stage statements as data (one file per skill or one stages table).
- `data/recipes.csv`: 386 recipes re-pointed row by row, by what each makes. Today 43 assembly and 29
  electronics recipes train engineering, 58 crafting and 18 smelting recipes train metalworking, 27
  cooking recipes train cooking, and 22 recipes train nothing. A script lists each row's proposed new
  skill for review before anything changes.
- The walk-up collect in `src/lib.rs` trains farming for every yield, stone included; it gets the
  yield's own skill (eggs and milk keeping animals, berries foraging, stone mining).
- Combat XP (`crates/humanity-systems/src/combat/mod.rs`) picks melee for kinetic kills and ranged for
  everything else; it gets the weapon's skill from a `skill` column in `data/equipment.csv`, and is
  earned from technique (section 7), not kills.
- Quest `xp_rewards`, `data/abilities.csv`'s `skill_required`, the syllabus's skill lists, and the
  activities catalog's `skill` field (becoming a list).
- Saves: nobody plays yet, so old skill ids are simply dropped (CLAUDE.md, no backwards
  compatibility before launch).

## 7. Three questions the catalog raised

### Ceramics

The kiln has nine recipes: fired brick, porcelain and ceramic composite train metalworking; the
planter, toilet and sink train woodworking; the floor tile trains shelter building; the two charcoal
recipes train nothing. Under "teach everything", pottery and kiln work is its own trade, so the
proposal gives it its own skill, `ceramics`: the seven clay recipes train ceramics; charcoal (burning
wood with too little air) trains `fire_making`; laying the bricks and tiles trains `masonry`; the
ceramic water filter trains ceramics to make and `water_purification` to use.

### Combat: offense and defense

The operator wants all forms eventually ("We want to have PvP arenas", PvE as humans against
non-humans, sport combat, PvP first to balance gear) and both offense and defense. So:

- **Weapon skills split by movement**: unarmed striking, grappling, blades, clubs and axes, staff and
  spear, archery, marksmanship, throwing, and gunnery for turrets and mounted weapons (point defense,
  fighter craft). Styles (boxing and karate; judo and wrestling) are tags until the game teaches the
  difference.
- **Defense is in every skill and also its own.** Real arts teach attack and defence together (a
  fencer's parry is fencing), so each weapon skill carries its guards. What no weapon owns
  (awareness, de-escalation, evading, escaping holds, wearing armour well) is `self_defence`, the
  renamed `defense`. `tactics` stays for coordinating a team.
- **XP from technique, not kills.** Range practice, drills and sparring train the skill (a clean parry,
  a grouped target, a throw landed in sparring); a kill is not the proof, because a count teaches
  nothing (research, ideas to avoid). PvP arenas are the sparring floor; sport combat adds its rules
  and protective gear; PvE invasions and point defense use the same skills; human raiders appear only
  as drills.
- **Real-life twin**: gradings and courses (a judo kyu grade, a self-defence course, a firearms
  safety course) recorded as certificates, as section 5 describes.

### Prop skills

The operator: a skill "by prop", because "learning to use a whip is way different from a staff",
with rifle drill in the same family. The family is **Flow and prop arts**: staff, poi, rope dart and
meteor, hoop, fans, twirling and drill (baton, rifle and sabre spinning; the fire axe and fire knife,
spun by the handle), juggling and whip, plus **fire performance safety**, which any lit prop trains
alongside its movement skill (fuels, wicks, dipping, spinning off, the spotter with the fire blanket).
Staff spinning declares a transfer to the fighting staff (`polearms`) and to twirling; poi to rope
dart. Juggling and whip are later rows until an activity teaches them (the fire props design builds
staff, poi, the fire axe, darts or meteor, fans and hoop).

Grading: the World Juggling Federation's levels (A7) are the pattern (countable feats, all or nothing,
judged on clear faults, feedback on failure), which a game can check from the motion itself. The
research found no grading body for poi or staff (A7, an absence of evidence rather than proof), so
the game's stage statements for those props would be the first of their kind, and the Skills page
should say so plainly.

## 8. The data, in one sketch

```
data/skills/skills.csv
id,name_key,family,description_key,transfers,max_level,xp_per_level
staff_spinning,skill.staff_spinning,flow,skill.staff_spinning_desc,polearms:0.25|twirling:0.25,30,100

data/skills/stages/staff_spinning.ron   (one file per skill: Infinite-of-X)
stages: [
  (stage: 1, from_level: 1, can: ["skill.staff_spinning.s1_a", ...],
   demonstration: (procedure: "staff_figure_eight", standard: "no drop for 20 turns"),
   topics: ["fire_safety_basics"]),
  ...
]
```

The curve (`base x level^1.5`, `data/skills/xp_curve.json`) stays the game's rule until the
operator chooses otherwise; it only fills the bar inside a stage.

## 9. Questions only the operator can answer

1. **The split rule.** Split where the movement or the knowledge is genuinely different, and only once
   the game can teach the difference, giving 90 skills in 11 families now (later rows added as their
   activities arrive)? *Recommendation: yes. It answers "teach everything" without rows that can
   never rise.*
2. **Stages as the visible measure.** Six named stages per skill with "I can" statements, numbers kept
   underneath for recipe gates? *Recommendation: yes; plain words beat a number, and the stages map
   onto real gradings.*
3. **The default mode.** Simplified by default (practice plus normal work earns the stage), the full
   assessment in the realistic mode? *Recommendation: yes, per the dual-modes rule.*
4. **Decay.** A stage is never lost; "rusty" and expiring certificates only in the realistic mode?
   *Recommendation: yes.*
5. **Testing out.** Anyone may attempt any stage's demonstration at any time and skip the practice?
   *Recommendation: yes; it rewards real knowledge and removes grinding.*
6. **Combat's granularity.** Eleven combat skills, or fold clubs and axes into blades as "melee
   weapons" and throwing into archery? *Recommendation: keep eleven; the movements differ and each
   has a real-world grading or club tradition.*
7. **Baton and rifle drill.** One skill (twirling and drill) or two? *Recommendation: one until the
   activity can tell them apart, with a declared transfer to staff.*
8. **The real-life record.** Replace the Tasks page's "XP for a real task" with the learning log,
   evidence and signed attestations, keeping XP for the game only? *Recommendation: yes.*
9. **The fantasy schools.** 94 rows of `data/abilities.csv` name eleven schools (pyromancy, cryomancy,
   arcana and the rest) that are not skills. Make them perk lines of real skills, or let fantasy
   abilities stop naming a skill and unlock by quest or setting instead? *Recommendation: the second.
   A fantasy school cannot be taught as a real skill, and the real skills should stay real.*
10. **Knowledge as curriculum.** Maths, physics, biology and the other knowledge areas tracked as
    curriculum topics passed by checks, rather than as skills? *Recommendation: yes (O*NET's split).*
