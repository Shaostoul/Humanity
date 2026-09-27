# Gardening, crafting, inventory and storage: what exists, what is missing

**Surveyed 2026-09-25** by four read-only passes over the code and data (after
the operator asked: "What could we do more gameplay focused? ... What else are
we missing to make gardening, crafting, and inventory more complete? Does the
stuff we have store anywhere physically?"). File references are as of that
date; re-check before trusting one. The container data basis is
[`docs/reference/findings/2026-09-25-container-materials-and-reuse.md`](../reference/findings/2026-09-25-container-materials-and-reuse.md).

## Progress

- **Defects 1-7: FIXED 2026-09-26** (v0.1344.0). Items return to their
  container when the backpack is full; a craft with no room is refused before
  spending anything and a finished one waits for room; crafting XP counts
  vessel-kept outputs; the showcase garden shows in the Garden panel and the
  sliders and field climate reach it; a new player starts with a kit from
  `data/world/player.ron`; every recipe input has a source, every plant has
  planting stock, the vendor sells real seeds, and
  `tests/recipe_sources_lint.rs` keeps it so; edibility, nutrition and
  spoilage come from `data/food/item_profiles.ron` (34 new profiles), not
  item-id prefixes.
- **Water is real: FIRST RUNG DONE 2026-09-26** (v0.1344.0). The Irrigation
  machine (`irrigates: true`, an `Irrigator` marker) waters grow areas only
  while powered, and draws each growing plant's `water_liters_per_day` from
  its plumbing island through the plumbing sim (real minutes, the same time
  base as the home's other machines). Rain waters outdoor fields. Hand
  watering takes 2 L from the fullest tank and is refused with a notice when
  the tanks are empty. The family home's water budget: production 2.40
  L/min, household 1.35, leaving about 1.05 for a showcase garden that needs
  up to 1.0 when everything is growing, so the cistern is the buffer. Still
  to do on water: filling a jug from a tank and pouring it back (part of the
  containers arc, where fluids become litres).
- **Containers remember and have materials: 3a + 3b DONE 2026-09-26**
  (v0.1345.0). See [containers.md](containers.md): memory of the last content,
  a Clean action that uses water, the toxic-history rule, a materials table
  and reactivity rules.
- **Stored goods are physical: DONE 2026-09-26** (v0.1346.0). The Barn's
  solid placeholder blocks are now open pallet racks (`ZoneFiller` mesh_kind
  "rack": posts and four decks, clamped under the 3 m ceiling), and what is
  filed in the Barn shows as crates on their decks, lowest deck first, one
  96 L crate per 96 L of stock (`src/engine/stock_piles.rs`). The default
  home's Barn starts with a homestead dry store (grain, flour, seed
  potatoes, fertiliser, lumber: 1,620 L, 17 crates) in
  `data/places/seed.json`. Every water tank carries a level gauge, and so do
  the bulk vessels (the grain silo, the fuel drums: `level_gauge` in
  home.ron), as two crossed plates readable from any side. The silo stood
  inside a rack bay and was moved to the Barn's clear east strip. Named
  bags, the garage and the car trunk are elsewhere and are not drawn in the
  Barn. Harvest the pack cannot take goes into a compatible vessel (the
  silo) and, when there is none, into the Barn (it used to be thrown away);
  automated machines file their output there too. Stock is drawn by kind since the same
  day: dry goods as sacks, liquids and fresh food as barrels, the rest as
  crates, each kind standing together.
- **Fluids are litres, first rung: DONE 2026-09-26** (v0.1347.0). Recipes
  that need tap water draw it from the tanks; bottles and jerrycans are
  filled at a tank and poured back; drinking a bottle returns it empty. See
  [containers.md](containers.md).
- **Crafting needs tools, and tools wear out: DONE 2026-09-26** (v0.1348.0).
  `data/crafting/tools.ron` names the hand tools a manual craft needs, by
  station and category (carpentry: hammer and hand saw; machine assembly:
  wrench and screwdriver; smithing: hammer; electronics: soldering iron and
  pliers) plus per-recipe lists (whittling and kitchen chopping: a knife;
  stone carving: chisel and hammer). 182 recipes need a tool. A tool is not
  consumed: it must be in the backpack, each craft wears it by one use, and
  it breaks at its items.csv durability ("Uses left" on its inventory card).
  Automated machines need none. A recipe that makes a tool never needs it,
  and a test proves every required tool can be had (vendor, kit, or a recipe
  whose tools can be had). The starter kit gained a hammer, hand saw,
  screwdriver, wrench and pliers. The statues no longer eat their chisel.
  The web Crafting page lists the same tools (a test keeps the two in step).
  A tool keeps its wear in storage: putting a worn tool away and taking it
  back no longer renews it (fixed the same day it shipped).
- **Stations draw power only while they work: DONE 2026-09-26** (v0.1349.0).
  The family home's stove (1.2 kW), oven (2.2 kW), electronics bench and
  sewing machine drew their full rating around the clock, about 3.65 kW of
  load that did no work. A Consumer's new `idle_watts` (data/machines/*.ron)
  marks a work station: it draws idle watts until a craft runs at it, then
  its working watts until the craft finishes (an automated machine while its
  own batch runs). A manual craft at an electric station is refused with a
  notice, nothing spent, while no machine of that type has power; the
  Crafting page says so too. Stations with no electrical role (workbench,
  fire-fed furnace) are unaffected. A craft whose station loses power
  partway pauses, with one notice, and carries on when power returns.
- **Crafting leaves byproducts, at real ratios: DONE 2026-09-26.** Five
  leftovers, each used by a recipe. Copper smelting leaves 6 kg of slag per
  2.8 kg ingot (about 2.2 t per t of copper), and slag stands in for half the
  gravel in a slag concrete. Sawing leaves sawdust (about 13% of the log,
  FAO) and milling wheat leaves bran (white flour is 72-76% of the grain).
  New oil presses for rapeseed, camelina, safflower and olives leave seed
  press cake and olive pomace. The composter turns sawdust with press cake
  or bran, and the pomace on its own, into fertilizer at C:N 20-40 with half
  the mass lost as a turned pile loses it (NRAES-54; Tiquia et al. 2002).
  Every ratio and its source sits beside its recipe in `data/recipes.csv`;
  `tests/byproduct_use_lint.rs` keeps every byproduct used, keeps those
  recipes from creating mass, and stops a recipe handing back more of an
  input than it took. Fixed on the way: tanning turns one raw hide into one
  leather (`leather_0`, which the leatherwork recipes now take) instead of
  two hides; the sawmill no longer cuts an 8 kg log into 10 kg of planks
  (it takes two logs); the grain mill grinds wheat, not paddy rice; the
  corn-seed oil recipe and the wheat-seed fertilizer are replaced. Added
  when merging (v0.1351.0): iron smelting leaves slag too (about 275 kg per
  tonne of iron, worldsteel); cheese leaves whey (about 3.4 kg of the 4 kg
  of milk), which bakes whey bread; pressing apples leaves pomace (about a
  quarter to a third of the fruit), which composts; and the juice press no
  longer makes 1.0 kg of juice from 0.8 kg of apples. Added 2026-09-26: the
  sawmill keeps its slabs and bark (FAO's split, 3 kg of slabs and 1 kg of
  bark from two 8 kg logs); slabs burn to charcoal at the log's yield, and
  bark tans leather at twice the hide's weight (Traditional Tanners), so
  leather now needs tannin as well as salt. Rubber (v0.1368.0): the rubber
  tree yields latex (it harvested a fiber bundle), 3 kg of latex sets into
  1 kg of raw rubber (dry rubber content 28%, Wattana et al. 2025), and
  vulcanizing cures 4 kg of it with 100 g of sulfur and heat into 4 sheets
  (it turned 2 sheets into 3); no recipe multiplies an item any more. Removed
  2026-09-26: three recipes that only multiplied an item and made nothing any
  recipe used (charging a battery pack into two, "titanium alloy" doubling
  titanium ingots, and "nanomaterial" turning one plastic sheet into five).
- **Automated machines fill the Barn, and rest when enough is on hand: DONE
  2026-09-26** (v0.1351.0). A machine's output used to land in the player's
  backpack wherever they were; it now goes into home storage (the Barn,
  where it shows as crates). A machine can carry `auto_keep` in
  data/machines/*.ron: it rests while that many of its product are on hand.
  The grain mill keeps 20 flour (whole grain keeps for years, flour for
  months, so it should not mill the whole store); the workbench keeps 2
  hammers. Without this the mill, now grinding wheat, would have milled the
  Barn's 400 wheat into the backpack.
- **Crafted goods have a quality grade: DONE 2026-09-26** (v0.1353.0). The
  six grades designed in `data/manufacturing.ron` (defective, poor,
  standard, good, excellent, masterwork) were parsed and unused; now a
  hand-made DURABLE good (anything with an items.csv durability) is graded
  by the crafter's level in the recipe's skill, with the file's own formula
  (skill factor plus a random -0.1..0.1). A level-1 crafter makes poor to
  standard work, a master good work or better. The grade sets how many uses
  a tool lasts (a defective one breaks at once, a masterwork lasts 2.5x)
  and what a vendor pays (defective: nothing). Materials and food stay
  ungraded, so stacks never split six ways; grades never share a stack;
  the grade survives storage; machines turn out standard goods. The
  inventory card shows the grade and the real uses left, and the Crafting
  page shows the grade to expect.
- **Every station can be built: DONE 2026-09-26** (v0.1357.0). The build_*
  recipes made station items (a forge, an anvil, a stove, an electronics
  bench...) that nothing could set down, so in the one-person home 13 of the
  stations recipes name never existed and about 160 recipes could not be
  made. Each now has a blueprint that consumes the crafted station item
  (`data/blueprints/basic.ron`), and `recipe_sources_lint` checks every
  station a recipe names can be built (the vehicle assembler stays a
  family-home machine by design). A built electric station (stove, oven,
  electronics bench, sewing machine) joins the home's power on its
  strongest island, idle until a craft runs at it, and refuses a craft
  without power, like a placed one (`wire_built_stations`).
- **Containers as items (3c): BLOCKED** on the unified placement schema
  (Tier B in PRIORITIES.md). The walk-up machine card and every vessel are
  tied to `data/machines/home.ron` placements; a container that can be
  picked up and set down needs placement to be one system first.
- **Gardening depth, first rung: yield from health and the unused plant
  models, DONE 2026-09-26** (v0.1350.0; the crop card now shows "Season
  health", which is what the harvest is scaled by). Each crop now keeps a season health record
  (`health_seconds` / `growing_seconds` on `CropInstance`, the time-average
  of its health while growing), and the harvest scales the rolled yield by
  it, linearly, the shape of the FAO-33 crop-water production function with
  Ky taken as 1 (`farming::harvest_quantity`). A crop kept well gets the
  full range; one that spent its season at half health gets about half,
  even if it has recovered by harvest day; a harvest of stressed crops
  posts one notice saying how much was lost. Dead crops still cannot be
  harvested. The six model sets are mapped in DATA, a `stage_models` table
  in `data/plants_visual.ron`: berry bush for raspberry, gooseberry and
  coffee; cactus for prickly pear; palm for coconut, date palm and palm;
  flower for tulip and four fictional flowers; grass for lemongrass, oat,
  millet, finger millet, teff and one fictional reed; mushroom for the two
  fictional fungi only (the model carries Amanita field marks, so the three
  edible mushrooms stay procedural); and the wheat set for spelt,
  triticale, rye and barley. 36 of 189 species now draw a stage model in
  beds and fields, up from 12. Still to do on gardening: N-P-K, pests,
  light, a per-crop yield response factor, season health in the Garden
  panel, and models for the species no set fits.
- **Gardening depth, rung 2: light, DONE 2026-09-26.** A green crop grows
  only while its grow area is lit and pauses in the dark; the dark never
  costs health, water or yield (`farming::light_growth_rate`). The sun is
  the solar panels' sun (up 6:00 to 18:00). Outdoor fields (`_field`, the
  test rain uses) have only the sun. Every other grow area has the sun too,
  through the skylight, because both shipped homes are designed sun-lit and
  place no grow light; a powered grow light (`lights_crops: true` on the
  `grow_light` machine in both home files, a `GrowLight` marker) also lights
  them after dark, and a shed or switched-off one gives no light. Lit time
  counts double so a natural day still grows exactly one day and
  `growth_days` stay true; a grow light left on all night therefore gives up
  to twice the growth, an upper bound until a per-species light amount
  exists. One powered light lights every indoor area (home-wide, like
  irrigation), which overstates a 100 W LED's reach of under a square metre.
  plants.csv gained `needs_light`, false only for the fungi (Penn State
  Extension: mushrooms "lack the ability to use energy from the sun"), so
  mushrooms grow in the dark. Still to do on light: per-light coverage, a
  per-species light amount (daily light integral) with saturation, seasonal
  day length, and showing the light state in the Garden panel.
- Found while merging: FIXED 2026-09-26 (v0.1354.0): the Eat and Drink
  buttons ask the food data (`food::consume_kinds`), so the water pump, the
  tester and empty bottles no longer offer Drink; `animal_fat_0` is made of
  tallow (a new material); 49 medical supplies, trees, medicinal plants,
  flowers and alien plants are refiled out of category "food"; `cook_coffee`
  roasts six cherries into a bag of beans instead of making energy drinks;
  tanning (fixed with the byproducts). NPC shops sold 16 items that do not
  exist; they now sell the real ones (`recipe_sources_lint` checks it), and
  `flask_0` became a real item. Soap is now made with lye (v0.1356.0, sold by
  the vendor; a real saponification ratio) and washing a food vessel takes a
  soap bar. STILL OPEN: eating one of anything counts as
  100 g, which is tied to satiation being an abstract 7-day reserve and
  belongs with the full-realism and simplified vitals modes rather than a
  quick fix; data/tech_tree.ron (not read by the game) names about 50 items
  that do not exist yet.
- **Gardening depth, rung 3: nutrients as N-P-K, DONE 2026-09-26** (v0.1355.0;
  merged with the soil saved in `WorldSave` and the crop card showing the
  grams held, the season need and what is short). Every
  unit of growing space (tower slot, bed, tray or field unit) holds grams of
  plant-available N, P2O5 and K2O (`CropSoil`, `farming/soil.rs`, which
  replaces the unused scaffold; numbers and sources in
  `data/garden/nutrients.ron`). A fresh unit holds 1.5 seasons of its crop's
  need, a stated game assumption. A crop draws its season need in step with
  its growth clock. The plants.csv N-P-K columns turned out to be relative
  indices, not kilograms, so one anchor fixes their scale: the tomato's
  published removal (1.5 g N per kg of fruit, CDFA FREP; 0.9 g P2O5 and 4.0 g
  K2O, UW-Madison A2809 Table 4.2), applied to each crop's expected harvest
  mass. When any nutrient falls below a tenth of a season's need, the
  scarcest one caps the crop's health (Liebig's law of the minimum). Health
  eases down at 0.1 a second to a floor of 20 and never reaches death: on
  Rothamsted's Broadbalk, unfertilised wheat still yields 10 to 40% of
  fertilised. Season health turns that into a smaller harvest. Fertilizing
  adds a bag's first-season nutrients instead of +40 health: compost gives
  1.3 g N (7% of 18.8), 6.4 g P2O5 and 11.6 g K2O per 2 kg bag (WSU
  Snohomish County Extension, 2016). The "nutrient" slider no longer
  multiplies growth. It now runs a feeder that tops each slot up from
  fertilizer in home storage, and half-way is just enough. A harvested unit
  keeps its worked soil for the next crop. Found, not fixed: at compost's
  real 7% first-season N, a 50-slot lettuce tower needs about 27 bags a
  season and the Barn holds 40, so compost alone cannot feed the towers.
  The indices understate grain and legume removal 5 to 20 times.
  Still to do: pH and humidity, the slow organic N that compost releases over
  years, urine and legume N, per-crop removal columns, and N-P-K in the
  Garden panel.
- **Gardening depth, rung 4: closing the nitrogen loop, DONE 2026-09-26.**
  Three real sources, each from cited numbers in `data/garden/nutrients.ron`
  and `data/plants.csv`. (1) Compost's slow release: the 93% of a bag's N not
  available in its first season is banked as organic N in its unit
  (`SoilMemory::organic`, saved) and released on the unit's garden clock by
  CDFA's rule applied to the WSU 7% first year (Gravuer 2016: halving each
  year to a 2% floor), so 3.5% of what is left in year two and 2% a year
  after. A bag applied every year gives 1.3 g of N in year one, 4.5 g by year
  ten and 12.4 g by year fifty of its 18.8 g: the soil builds up. (2) Urine:
  `urine_stored_0` is one person-day (1.5 kg: 10.9 g N all available, 2.3 g
  P2O5, 3.6 g K2O; Jonsson et al. 2004, EcoSanRes, Tables 1 and 3). It
  collects in a 20 L sealed tank on the waste meter's real clock and the
  Compost action draws whole person-days off. The feeder uses urine first,
  dosed for N, then compost for P and K; the Fertilize button uses compost
  first and urine when there is none; urine never goes on a crop within 30
  garden days of harvest (WHO 2006, as in Richert et al. 2010). (3) Legumes:
  plants.csv `n_fixed_pct` (soybean 55, Salvagiotti et al. 2008; fava 67, pea
  54, lentil 54, chickpea 52, common beans 26, Hossain et al. 2017; the other
  legumes blank until sourced). A legume draws only its unfixed share and
  leaves its roots' fixed N for the next crop, sized so soybean leaves its
  unit where it found it (Salvagiotti's near-neutral balance), fava richer,
  common bean poorer. The balance, by the game's own need model: the home
  zone's showcase garden (1,600 units) removes 10.35 kg of N a garden year and
  its legumes return 0.08 kg. Three residents' urine is 12.0 kg a year, so on
  one shared clock urine alone covers it with 1.8 kg over; the one player the
  game has covers 39%. P2O5 and K2O do not close from urine (8.7 and 23.9 kg
  needed against 2.5 and 4.0). Found, not fixed: the body and the garden run
  on different clocks. Vitals, waste and urine run on real seconds (v0.1005),
  the garden on 20-minute game days at the 10x growth default, 720 garden days
  per real day, so in play one player's urine is about 0.05% of the garden's
  draw (0.5% at 1x). Closing that is a design choice (bodily outputs on the
  game clock, or a slower garden), not a number to tune. Also found: the
  waste meter composts 1.8 bags (34 g of N) a real day, 2.7 times the N in a
  person's whole excreta (12.5 g a day, Jonsson Table 1), and the plants.csv
  indices make grain and legume removal (and so legume credits) far too
  small; per-crop removal columns are the fix for both.
- **Gardening depth, rung 5: pests and integrated pest management, DONE
  2026-09-26** (v0.1361.0, with each grow area's pest levels and control
  buttons in the Garden panel, and Off / Gentle / Realistic in Settings). Five pests in `data/garden/pests.ron`, each with its hosts
  (plants.csv ids), where it lives, what favours it, how fast it multiplies
  and how much it can cost, all cited: aphids (UC IPM 7404: 80 offspring a
  generation, favoured at 65-80 F and by excess nitrogen, "large populations
  can turn leaves yellow and stunt shoots"), spider mites (UC IPM 7405 and
  Iowa State: "70-fold in as little as 6 days", hot and dry, 40-60% soybean
  loss untreated), cabbage caterpillars (UMN, outdoors in the growing season),
  slugs (UC IPM 7427, outdoors, damp or dark) and the Colorado potato beetle
  (UMN, Cornell, UKY: outdoors, warm, and it waits in the soil where potatoes
  grew). Each grow area holds a pressure per pest (`SoilMemory::pests`, so it
  is saved and stays with the place between crops) that grows logistically
  from a trickle of arrivals, faster on a monoculture of its host and in the
  conditions it likes, and fades with no host, which is why rotation works:
  a season of wheat where potatoes grew halves the beetles every 30 garden
  days. Pressure caps a host's health like a nutrient shortage (gently, never
  below 20), so a neglected infestation costs season health and yield, not
  the crop. The player is told once when a pest appears, with what favours it
  and its controls gentlest first; the controls, in the IPM order, come
  through a new `pest_control_request` channel: hosing off (half a litre a
  plant from the tanks), hand-picking, Bt for caterpillars and predatory
  mites that keep eating for days while there are spider mites (UC IPM), then
  insecticidal soap (a bar of soap in 5 L of water, 2%, Clemson's "1 to 2%";
  soft-bodied pests only, per CSU, and it kills the predatory mites) and iron
  phosphate slug bait. No synthetic pesticide. Three modes by
  `garden_pest_severity`: off, gentle (the default, half the damage) and the
  cited damage. The Garden panel buttons, pressure display and Settings mode
  switch shipped in the same release. Whitefly, western flower thrips and a
  floating row cover followed on 2026-09-27 (sticky cards, Encarsia and
  cucumeris mites as their controls; a covered field is shut to wild insects,
  so it is pollinated by hand until the cover comes off). Pests still do not
  advance during the offline catch-up, by design.
- **Gardening depth: per-crop nutrient removal, DONE 2026-09-26.** plants.csv
  gained `removal_n_g_per_kg`, `removal_p2o5_g_per_kg` and
  `removal_k2o_g_per_kg`, filled for 64 crops: every crop in the family home's
  beds and nutrition towers except flax and the mushrooms (14 of the
  apothecary tower's dried and medicinal herbs are still blank), each value
  cited to its table cell
  in `data/garden/nutrients.ron` (REMOVAL COLUMNS). Grain, dry pulses and
  oilseeds take N from USDA NRCS's Agricultural Waste Management Field
  Handbook Table 6-6 and P2O5 and K2O from UW-Madison A2809 Table 4.2; fresh
  produce takes N from the nitrogen USDA measured in the food (FoodData
  Central SR Legacy protein over its nitrogen factor, which matches CDFA's
  measured tomato removal within 6%) and P2O5 and K2O from A2809, else
  FoodData Central. A filled column is the need (removal x expected harvest,
  `soil::removal_per_kg`); a blank one falls back, nutrient by nutrient, to the
  old anchor-scaled index. A wheat unit now needs 146 g N, 58 g P2O5 and 41 g
  K2O a season (was 7, 5 and 8.4), and legume fixation and credits now work on
  real N. The home garden by the rung-4 method (1,600 units, seasons a garden
  year from growth days): N removed 71.7 kg (was 10.35), drawn from the soil
  after fixation 57.6 kg, returned by legumes 11.5 kg (was 0.08), net 46.1 kg
  (was 10.2); P2O5 25.0 kg (was 8.7); K2O 53.2 kg (was 23.9). Three residents'
  urine (12.0 kg N, 2.5 kg P2O5, 4.0 kg K2O) covers 26% of the net N (was
  118%), 10% of the P2O5 and 7% of the K2O; one player covers 9% of the N. So
  rung 4's "urine alone covers it" was an artifact of the indices: that garden
  removed 10.35 kg of N from its soil while its own harvest held about 80 kg.
  A shipped-data test holds every value to a physical range and within 0.65
  to 1.4 times the N in the crop's protein, which a lb-per-ton slip fails.
  Found, not fixed: items.csv and `data/food/crop_nutrition.ron` disagree on
  what a harvest weighs (a wheat item is 500 g, a wheat yield unit 50 g; a
  bean item 300 g, a unit 50 g of dry seed), and the need follows items.csv,
  so grain and pulse units remove 6 to 10 times what the food model says they
  feed (a tower cup grows 1.65 kg of dry soybeans a season; the rice trays
  alone remove 9.7 kg of N a year). Shipped with it (v0.1363.0): the crop
  card's "Each kg takes" row shows the grams of N, P2O5 and K2O a kg of that
  harvest carries out of the soil (it had carried the old unitless index,
  shown nowhere), and the showcase oil bed grows sunflower instead of flax,
  which is modelled as a fiber crop. In progress: a grow unit holding as many
  plants as its floor area fits at the crop's spacing, with real per-plant
  yields, which fixes the harvest-mass disagreement above. Still to do: the
  remaining crops (fruit trees, spices, dried herbs, fiber) and mushrooms
  drawing on their substrate.
- **Grow lights light the plots near them, and beds are divided into plots,
  DONE 2026-09-26** (v0.1364.0). A powered grow light used to light every
  indoor grow area in the home. Now it covers the canopy its photons can
  serve, nearest machines first, within 3 m across the floor (a game
  estimate), and a machine it only partly covers grows that share of a
  night's growth (`farming/lighting.rs`, `data/garden/lighting.ron`). A light
  puts out watts x 2.30 umol/J (the DLC Horticultural V3.0 minimum for a
  listed LED fixture) and a lit plot needs Cornell's 17 mol/m2/d lettuce light
  integral delivered through the 12-hour night, 394 umol/m2/s, so a 100 W
  light covers 0.58 m2: 2.1 kWh per m2 a day, which matches the 2.2 kWh per
  m2 a day self-sufficiency.md gives greens. A tower's canopy is its cups at
  Cornell's finishing spacing, 38 plants a m2. The engine publishes the home's
  grow machines with their positions ("grow_plots"). Each grow medium now
  divides its machine into `plots` (a 2 x 1 m bed or tray into two 1 m2
  plots, a field into four, a mushroom rack into four shelves), one crop per
  plot, and every crop is tagged with the machine it stands in. That fixed a
  defect: the Garden panel's bed Plant button tagged crops with the machine
  TYPE, so they were never drawn in the world (BUG-089). Found, not fixed: the
  renderer draws one plant per plot, so a 1 m2 plot of wheat shows one stalk;
  it should draw the plot's plants at the crop's spacing once that lands.
  Still to do: each species' own light need (fruiting crops want more than
  17, some lettuces tip-burn above 15). Added after: each crop's card shows
  what is lighting it now (the sun, the sun through the skylight, a grow
  light and what share of the plot it covers, or dark until sunrise). Pests
  are not advanced by the offline catch-up, on purpose: the offline design's
  rule for anything that harms without a decision (offline-progression.md).
- **Gardening depth: soil pH, DONE 2026-09-26** (the Garden crop card shows
  the unit's pH against its crop's window, each soil grow area has Lime and
  Sulfur buttons, and Settings has "Soil pH: Off / On"). plants.csv's
  `ph_min`/`ph_max` windows, read by nothing before, now matter. The model is
  `src/systems/farming/soil_ph.rs`; every number and its source is in
  `data/garden/soil_ph.ron`. Each soil unit (bed, tray or field unit) has a pH
  kept in `SoilMemory::ph`, so it stays with the unit between crops and is
  saved (an older save reads every unit at its medium's start). A bed starts
  at 6.5 (OSU EC 1560: "vegetable gardens produce well in a soil pH of 6.5");
  a tower's nutrient solution is held at 6.0 by its dosing (UF/IFAS HS796:
  "Final solution pH should be in the range of 5.8 to 6.2") and never drifts;
  mushroom substrate is not modelled. Everything that moves a pH is counted in
  grams of calcium carbonate equivalent over the loam's buffer, 381.1 g of
  CaCO3 per m2 per pH unit (UC Cooperative Extension, Vossen, Table 1, loam,
  5.5 to 6.5, 7-inch layer; Purdue HO-241-W's and Clemson HGIC 1650's sulfur
  tables give 305 to 381 through sulfur's 3.12), and reaches the pH over garden
  days at a half-life per pool. Stored urine acidifies as urea does, 1.8 g of
  CaCO3 per g of N (UNL G1503 Table 1; Purdue Table 2 gives 1.76; Neina and
  Dowuona 2013 measured urine's pH fall "attributed to nitrification"), with a
  14-day nitrification half-life (an estimate on UMN's "rapidly proceeds in
  warm, moist, well-aerated soils"); compost is taken as neutral (the low end
  of Purdue's "0 to 10+"). Lime is 100% CaCO3 equivalent (UC Table 2) with a
  30-day half-life (Clemson: "two to three months before planting"); sulfur is
  90% S at 3.12 (Purdue Table 2) with a 60-day half-life (OSU EC 1560: "apply S
  in the fall and test the soil pH in the spring"), capped at 97.6 g per m2 an
  application (OSU EC 1560 Table 2). Each button brings every planted unit to
  the middle of its crop's window, counting what is still reacting, and takes
  the bags from the backpack: garden lime (5 kg) and garden sulfur (2 kg) are
  new items, sold by the vendor and the Farming Elder. Outside its window a
  crop's health is capped at 30% per pH unit, the median of USDA NRCS's "Soil
  Quality Indicators: Soil pH" (2011) Table 1 points against the plants.csv
  windows (a test recomputes it), never below 20, the lowest of the nutrient,
  pest and pH caps binding; the player is told once per bed which amendment
  fixes it. Checked: OSU's lawn example (3.5 lb N per 1,000 sq ft a year as
  ammonium sulfate, "0.1 to 0.2 units each year") runs at 0.24 a year in the
  model, the fast end; OSU's own high-organic-matter loam needed about three
  times the UC buffer. Eleven tests, each seen red on a deliberate break.
  Merged as v0.1366.0, when a plot of a placed bed started buffering over its
  real floor (the engine's "grow_plot_area_m2"); the area inferred from the
  crop's N removal is now only the fallback for a unit with no machine.
  Left: wood ash is
  skipped because burning gives the player no ash item yet (FIXED
  2026-09-27: charcoal fires leave it and it limes, see below); crop removal of
  calcium and magnesium, and legumes, which also acidify (NRCS), are not
  counted; hand-planted crops have no unit, so they sit at 6.5 and cannot be
  limed; a tower's solution cannot yet be set per crop (a blueberry wants a
  lower one). FIXED 2026-09-27: pH now steps over the offline catch-up (the
  lime, sulfur and nitrifying ammonium still reacting when the player left
  keep reacting, `soil_ph::hand_away_secs`; offline-progression.md), and the
  Home page's tower check no longer intersects the plants.csv SOIL windows:
  it shows the pH the solution is held at, 6.0, the setpoint the model grows
  towers at (both shipped towers had shown a shared window of exactly 6.5).
- **Gardening depth: what a grow unit really harvests, DONE 2026-09-26.**
  A bed, tray or field plot now holds as many plants as fit
  (`max(1, floor(plot area / area_per_plant_m2))`), a tower cup and a
  hand-planted crop hold one, and plants.csv yield_min/yield_max are a cited
  harvest PER PLANT in items of the harvest item. The plot area is the
  engine's `grow_plot_area_m2` (footprint over plots); `area_per_plant_m2` is
  a planting guide's in-row x between-row spacing, or 1 / the recommended
  plants per m2 for drilled grain and pulses. 52 crops are sourced, every one
  the home plants except mint, rosemary, aloe vera, St. John's wort and the
  oyster mushroom (left at one plant a unit, each named with its reason), in
  the new `data/garden/yields.ron`: NCSU CEFS's planting guide and UMN's
  per-plant yields for vegetables (high-tunnel figures for fruiting crops),
  NASS 2023-2024 yields with UMN, NDSU, Montana State and IRRI plant densities
  for grain, pulses, sunflower and potato, Stapleton and Hochmuth's vertical
  tower herb trial, Montana State's herb trials, and more, each with its
  table, link and read date. The harvest roll, the season nutrient need (so
  a fresh unit's store and the feeder's target), the legume credit and the
  irrigation draw all scale with the plants (`farming::units`). Water per
  plant is now FAO-56 crop ET (season-average Kc x 3 mm/day x the plant's
  area) for 30 crops, and the small-vegetable curve as an estimate for 12
  more that were over 3x off (a wheat plant 0.6 L/day was 80x too much; now
  0.0077). One weight for a harvest: crop_nutrition.ron's
  `grams_per_yield_unit` is gone, the food model weighs by the items.csv item
  and the plants per plot, and the legume items say they are dry seed (green
  peas excepted) with USDA bushel-weight volumes. Before and after: a 2 m2
  wheat tray 7.0 kg of grain needing 146 g N a season, now 666 plants giving
  0.66 kg and needing 13.8 g; a 2 m2 rice tray 6.0 kg and 83 g, now 66 hills,
  1.71 kg and 23.7 g; a soybean tower cup 1.65 kg of dry beans removing 103 g
  N, now one plant, 14 g and 0.86 g; a tomato cup 1.0 kg and 1.5 g, now one
  high-tunnel plant's 6.35 kg and 9.5 g. The home garden by the rung-4 method
  (1,300 tower cups and one plot a bed; the showcase's six units a bed give
  within 3%): N removed 15.9 kg a garden year (was 71.7), drawn from the soil
  after fixation 15.8 kg (was 57.6), returned by legumes 0.1 kg (was 11.5),
  net 15.7 kg (was 46.1); P2O5 6.5 kg (was 25.0); K2O 22.1 kg (was 53.2).
  Three residents' urine now covers 77% of the net N (was 26%), 39% of the
  P2O5 and 18% of the K2O; one player 26% of the N. On the same basis the
  garden supplies about 6,300 kcal a day (was 28,400). A shipped-data test
  holds every crop's harvest per m2 to a band for its kind of harvest (dry
  seed 0.05 to 1.5 kg/m2, and so on), so a per-m2 figure typed as per-plant
  fails, and holds plants.csv to the cited kg through the items.csv weight.
  Found, not fixed: (1) season compression: growth_days is days to the first
  pick and the game picks once, while a picked or cut crop's yield is its
  whole season, so a cup replanted back to back yields about 2 to 3 times a
  real tomato's year and 9 times a tower basil's; the next rung is a picking
  window. (2) home.ron's potato bed "+120 kcal/d" is about twice the cited
  yield (61.5 kcal/d cropped back to back). (3) The 5.76 m2 grain field
  really gives about 54 kcal/d of wheat against its "+3100 kcal/d" (the
  outline's 660 m2 finding stands). (4) A harvest still returns 2 seeds a
  unit whatever its plants, and pest pressure still counts units, not
  plants. (5) Hazelnut and walnut carry kernel nutrition on in-shell items.
- **Pollination decides how much fruit and seed a crop sets: DONE
  2026-09-26.** Until now an indoor tomato set a full crop with nothing to
  shake its flowers. `data/garden/pollination.ron` lists 21 crops by
  plants.csv id: how each is pollinated (self, vibration, insects or wind),
  its flowering stages, the share of a full crop it sets indoors with no help,
  and how often a hand pollination must be repeated, each with a quoted,
  dated source (mostly McGregor's USDA Agriculture Handbook 496, with UMD,
  UF/IFAS, NDSU, Purdue, Ohio State, Abak 1995 and Klatt 2014). Tomato 0.49
  (Moore 1968: 4.3 against 8.8 pounds in a plastic greenhouse), pepper 0.65,
  eggplant 0.81, strawberry 0.21 (Allen and Gaede 1963, an undisturbed
  greenhouse), sunflower 0.67, cucurbits nothing, corn 0.5 (a game estimate:
  no source measured still air); beans, peas, the pulses, okra and the
  grains pollinate themselves. The model (`farming/pollination.rs`): an
  indoor crop that needs help records its flowering days and how many were
  pollinated, by a hand pollination (the Garden panel's Hand-pollinate button
  on an area with flowers waiting, "pollinate_request") or by a bumblebee
  hive, a new catalog machine (`bumblebee_hive`, not placed in the seed
  homes) whose bees reach the indoor grow areas within 17.8 m, the radius of
  the 1,000 m2 one colony serves (Ohio State: 7 to 15 colonies a hectare);
  bees do nothing for corn. Outdoor fields need no help. At harvest the fruit
  set is ONE multiplier on the yield. The player is told once when an area
  starts flowering with nothing to pollinate it, and the crop card gains a
  "Pollination" row. Settings has Pollination Off / On (On by default; Off
  sets every crop fully and shows nothing). The record is saved with the
  crop. Since 2026-09-27 a hive needs a bought colony, which works 70 garden
  days (Koppert: remove hives "the latest 10 weeks after introduction") and
  is then spent, and a circulation fan sets 0.79 of a strawberry crop
  (McGregor 1963). Not modelled yet: LSU warns a small garden's few flowers can leave the bees
  over-working and damaging them. Found, left alone: the harvest returns two
  seeds whatever the fruit set, so an unpollinated zucchini still gives
  seed; and a crop the offline catch-up carries past flowering is not
  charged for flowers nobody pollinated while the game was closed.
- **Picked crops are picked over a season, and seed follows the harvest:
  DONE 2026-09-26.** A tomato, pepper, cucumber, zucchini, eggplant, okra,
  strawberry, kale or cut herb no longer hands over its whole season at its
  first ripe fruit. `data/garden/harvest_windows.ron` says, for all 56 crops
  the homes grow, whether each is harvested once or picked, and for the 20
  picked ones the window and interval, each quoted with its link and date:
  UMN's planting tools (the page whose per-plant yields yields.ron already
  uses) for the fruiting crops and kale (tomato "Indeterminate: 4x per week
  over 8-10 weeks", 36 picks; pepper 21; eggplant 18; cucumber 21; zucchini
  14; kale 12; high tunnel strawberries "Daily over 16 weeks", 112),
  Stapleton and Hochmuth's weekly-cut tower herbs from each herb's first cut
  to mid-June (basil 38 cuts over 266 days), Montana State's herb trials
  (chamomile 4 rakings 10 days apart, calendula 12 pickings 5 days apart,
  feverfew 2 cuttings a month apart), India's NHB for lemongrass (5
  cuttings 65 days apart) and USU for chives; okra's window, chive's spacing
  of cuts and comfrey's interval are labelled estimates. The model
  (`farming/picking.rs`, a `CropPicking` on the crop, saved in `WorldSave`):
  once ripe, a picked plant's season is split into equal shares, one coming
  ripe each interval; Harvest takes the ripe shares, pressing again before
  the next gives nothing, and the season is rolled once per plant with
  whole items out and the fraction carried, so a plant picked every time
  gives exactly its cited season, scaled by season health and fruit set.
  After its last share the plant is spent and its plot empties (keeping its
  soil), with a notice; a Clear button pulls a ripe picked plant early. Two
  modes in Settings, "Picking": Forgiving (default), where ripe produce waits
  on the plant, and Realistic, where a pick not taken before the next comes
  ripe goes past its best and the plant is spent when its window ends. The
  crop card gains a "Picking" row ("35 of 36 picks left, next in 1.2 garden
  days"), and "ready" and the bulk "Harvest N ready" button mean a pick is
  ready. The home food model (`self_sufficiency::food_supply_kcal_per_day`)
  now counts a picked crop's season once per growth days plus window, not
  once per growth days: a tower basil cup a season per 296 days, not per 30.
  Seed: a survival harvest returns 2 seed items for a full season (a game
  rule) in proportion to what it harvested, so a zucchini nothing pollinated
  gives no seed and a pick gives a share. Found, not fixed: the farming tick
  skips a ripe plant, so through its window a bearing plant takes no water,
  no nutrients and no stress; for tomato, pepper and cucumber the season
  total is a high tunnel's and the window a field crop's (UMN's timing table
  has field rows only), so the total is right and the daily rate high; and
  mint, rosemary, aloe vera, St. John's wort and the oyster mushroom stay
  harvested once until their yields are sourced.

- **Each crop's own light need, and a grow-light timer, DONE 2026-09-26**
  (v0.1368.0). plants.csv gained `dli_min`, `dli_target` and
  `dli_saturation` (mol/m2/day) for the 27 of the home's 63 crops the
  research found (docs/reference/findings/2026-09-26-crop-daily-light-integrals.md:
  Purdue HO-238, Cornell CEA, Michigan State, NASA and peer-reviewed work;
  the rest are listed there as not found). A grow light's area is now quoted
  at the lettuce reference (17) and a plot takes its neediest crop's target
  over 17 times the canopy, so the same 100 W light fully covers 0.58 m2 of
  lettuce and 79% of a 0.5 m2 tomato plot (25). The lights run on a timer
  (`lamp_photoperiod_h`, 18 h): on at sunset, off at midnight, because
  "Some crops, especially tomato, become stressed and develop chlorotic
  leaves if grown under continuous light" (Runkle, MSU), so a lit night is
  now half a day of extra growth, not a whole one. Each crop card shows its
  light need, or says it is not sourced and planned at 17. Still to do: the
  sun side (the skylight is treated as meeting every target; there is no
  latitude or seasonal day length), `dli_min` and saturation in the growth
  rate, a timer the player can set, and lettuce's still-air tipburn ceiling
  (12, not 17) once the greenhouse has airflow.

- **Greenhouse humidity is real, and fungal disease follows it: DONE
  2026-09-26.** Each indoor grow room (the room box a grow machine stands
  in) now holds its own water vapour (`farming/humidity.rs`, every number in
  `data/garden/humidity.ron` with its quoted, dated source). It gains what
  its growing, watered crops breathe out: plants.csv's litres a day per plant
  times the plants in each unit, all of it, because that column is FAO-56
  crop evapotranspiration ("Nearly all water taken up is lost by
  transpiration"). It loses vapour to the home's air at the room's leakage,
  0.5 air changes an hour (UGA B792 Table 2, new double-layer film "0.5 to
  1.0", out of the wind), plus any exhaust fans, solved exactly per tick on
  the game clock and capped at saturation. Relative humidity uses FAO-56's
  equation 11 and Annex 3 (287 / 0.622). THE BALANCE: the 3-person
  greenhouse (2,970 m3) at full planting breathes out about 540 L a day and
  would settle above saturation on leakage alone, so the home now carries an
  `exhaust_fan` there: a 12 inch EC inline fan with a humidity controller
  (AC Infinity CLOUDLINE T12, 1604 CFM = 2,725 m3/h, 250 W) that holds the
  room at 80% (five points under UMass's 85% gray mold line) at about half
  speed, drawing about 30 W by the fan laws' cube. The one-person greenhouse
  (about 200 L a day) settles near 68% on leakage and carries none. Three
  diseases joined `data/garden/pests.ron` as rows of the pest model, each
  with a new `Humidity` window and its own `unfavoured_rate` (0.05, because
  the sources say they infect only in their conditions): gray mold (85% and
  up, 12.8 to 23.9 C, a 6-day cycle, up to half the yield), powdery mildew
  (from 50%, it needs no wet leaves, so ventilating does little; 5 days; 0.3)
  and downy mildew (above 85%, 14.4 to 25.6 C, 7 days, 0.7). Spider mites
  gained the "humidity is less than 90 percent" their Iowa State source
  already gave. Controls in the IPM order, with a new Cultural kind first:
  Ventilate (a heat-and-vent: one air change of the room at once, which the
  crops breathe back within an hour or two, as UMass says), Remove infected
  leaves, then Sulfur (`garden_sulfur_0`, a protectant for 7 days, refused
  where a cucurbit grows, per its label) and Potassium bicarbonate (a new
  bought item, eradicates powdery mildew and guards against gray mold and
  downy mildew for 7 days). A crop outside its plants.csv humidity window is
  capped gently, at most 10% (Bakker 1991) 25 points outside it. The Garden
  panel shows each area's air, its room and its fan, and each crop card a
  "Humidity" row; the Settings pest severity now reads "Garden pests and
  diseases" and covers them. The room air is saved in the soil memory; old
  saves load with none. Found, left alone: (1) home.ron's Energy loop text
  (12.0 against 12.2 kWh a day) does not count the fan's ~0.8 kWh a day,
  and had only 0.2 kWh of headroom. (2) The greenhouse exhausts into the
  home air, which the atmosphere model holds at 40%: a real home would
  condense that water in its air handling and could return ~540 L a day to
  the tanks; neither is modelled. (3) The crops breathe their daily water
  evenly day and night, and the grow rooms sit at a fixed 21 C. (4) Oyster
  mushrooms want 85 to 95% and the mushroom room sits near 50%, so they are
  gently capped; a humidifier is the real answer. (5) Crowding is not
  modelled: every plant is sown at its cited spacing.

- **The mushroom room is humid enough for mushrooms to fruit: DONE
  2026-09-26.** A `humidifier` machine in both home catalogs (new
  `MachineDef::humidifies_l_h`, spawned as a `Humidifier` component): an
  ultrasonic grow-room humidifier with an onboard controller, AC Infinity
  CLOUDFORGE T7 (1300 ml/h, 100 W, "PID control", an "Automatic water valve
  with tubing refills from an external water source"). Its controller
  (`humidity::humidifier_share`) runs it flat out below its setpoint, at the
  output that holds the setpoint there, off above it; it draws 100 W times
  its output share (a stated game choice: the maker publishes only the
  full-output draw). The setpoint is 90%, UF/IFAS SS662's fruiting chamber
  ("90% humidity controlled by humidifiers or foggers"). Its water is real:
  it runs on the crops' own water gate (the tanks not dry, the irrigation
  running), and `step_rooms` returns its litres, which the tick adds to
  `irrigation_demand_lpm`, so the plumbing takes them from the tanks and a
  dry cistern stops it. In a humidified room the fans work five points above
  its setpoint and no damp-disease notice is given. Fungi (plants.csv
  `needs_light` false) below their window now lose as the square of the
  shortfall, `(d / 0.47)^2`, fitted to Kim et al. 2013's king oyster yields
  (4.4% lost 10 points under the best, 18.2% 20 under), never below the
  house floor; green crops keep Bakker's gentle cap. THE BALANCE: the family
  home's mushroom room (300 m3, six racks, about 7 L a day breathed out) sat
  at 48% (oysters held to about 38); with `humidifier_1` it holds 90.0% at
  88% output, about 88 W, 2.1 kWh and 27 L of water a day. The solo home's
  room (two racks) sat at 41% (oysters at the floor); its humidifier runs flat
  out, 100 W, 2.4 kWh and 31 L a day, and the room settles at 88.3%, inside
  the oyster's 85 to 95%. The Garden panel's line says what it is doing
  ("humidifier at 88%, 88 W, 27 L of water a day", or "stopped: no water
  from the tanks"). RoomAir saves its output, litres and dry flag; old saves
  load it idle. FOUND, and written into both homes' loops rather than left
  quiet: humidifying a whole room is expensive. The family home's Energy
  loop is now about 14.8 kWh a day against 12.2 (two more panels would close
  it), and the SOLO home's Energy loop NO LONGER CLOSES (about 6.4 against
  5.76 kWh; a fifth panel would close it), both for roughly 100 to 300 kcal
  of mushrooms a day; both Water loops gain the humidifier's litres (the
  cistern's dry-season reach drops from 33 to 30 days and from 100 to 72).
  The real answer at this scale is a fruiting tent around the racks, a few
  cubic metres instead of 300, which the model could carry as a small room
  box of its own (the AirMap already picks the smallest box a machine
  stands in) but no machine publishes one yet. Also left: the T7's own 15 L
  reservoir is not modelled (it stops with the tanks); its vapour leaves
  into the home air, still held at 40%; and the humidifier's litres are
  billed on the irrigation's per-real-day clock, like the crops' water.
- **Weeds compete with the crops in soil, and the player keeps them down:
  DONE 2026-09-26.** Every soil grow area (beds, trays, fields and the
  hand-planted crops; `soil_ph.ron` decides which are soil, so towers and
  the mushroom racks have none) now holds a weed cover and a seed bank in
  the saved soil memory (`farming/weeds.rs`, every number in
  `data/garden/weeds.ron` with its quoted, dated source). Weeds come up from
  the bank on the garden clock and reach half cover on bare soil in about
  three weeks; an outdoor field starts with a typical field's bank
  (eOrganic: "thousands of weed seeds ... per square foot") and a filled
  indoor bed with a quarter of it (UMass greenhouse guide: "using sterile
  media"), so fields get weedy faster. The bank runs down to 5% in five
  clean years and one uncontrolled season fills it back to about 90%
  (Burnside 1986, via eOrganic), so letting weeds flower costs years. Cover
  caps crop health like a pest, hardest in the crop's critical period: the
  cited windows for tomato (UMass, 3.3 to 5.8 weeks), onion (NC State, 4 to
  6 weeks), carrot (UC IPM, the first four weeks), dry beans, sweet potato
  and wheat (Agostinetto 2008, days 12 to 24), FAO's "first one-third of the
  crop growing cycle" for the rest; the loss from weeds left all season is
  WSSA's table per crop (corn 50%, spring wheat 19.5%, dry bean 55.3% and
  so on; its median, 47%, for crops it does not list; onion capped at 0.8
  under NC State's 96%). A quarter of the cap applies before the window and
  half after it. Controls in the order growers are taught: Hoe (the hoe the
  player starts with, one use of wear per 10 m2; 90% of small weeds, NEVG's
  60% once they are big), then Mulch with sawdust (Oklahoma State: 50 lb on
  100 sq ft, a season) or bark (Clemson: 2 to 3 inches, 2 to 4 years), both
  from the sawmill; each stops 90% of new weeds and draws 20 g of nitrogen
  per kg from the soil under it as it rots (Oklahoma State's "1 pound of
  actual nitrogen per 50 pounds"), which is banked as slow organic N. The
  Settings pest severity now reads "Garden pests, diseases and weeds" and
  covers them (Off / Gentle / Realistic). The Garden panel shows each soil
  area's weed row (cover, mulch days left, seed bank) with Hoe and Mulch
  buttons; the crop card a "Weeds" row when they cap it. Found, left alone:
  (1) the harvest follows the season's average health, so an untouched bed
  loses about half the cited season-long figure (roughly 20% for a tomato
  against WSSA's 47%). (2) The game has no straw, the usual vegetable
  mulch, and both mulches it can make tie up nitrogen (Cornell does not
  recommend bark in vegetable beds). (3) No source read gave the share of a
  bed's nutrients weeds take, so their competition is all in the health
  cap. (4) Perennial weeds, and weeds as a home for pests, are not
  modelled.

- **Every mushroom rack fruits in its own tent: DONE 2026-09-26.** The
  fix the humidifier bullet above named. The rack's grow medium (`grow_media.ron`) now carries an
  `enclosure`: its shelving wrapped in plastic at 1.3 x 1.9 x 0.7 m (1.73 m3),
  holding ten 5 lb blocks (22.7 kg; a Martha tent's "8-12 fruiting blocks",
  Nature Lion 2026). It is a property of the medium, not a tent machine,
  because it is the same data path that already divides the rack into shelves
  and needs nothing placed around each rack. `humidity::AirMap` makes each
  tent a room of its own inside the room the rack stands in: the tent
  exchanges air with that room, and everything it vents becomes that room's
  vapour, which the room leaks to the home. THE TENT IS NOT SEALED: its fresh
  air is set by its substrate's CO2. That is 1.09 g of CO2 per kg of
  substrate an hour, the fruiting-stage rate in Pavlik et al. 2020. The limit
  is 1,000 ppm, since above 0.1% CO2 "produces a toxic effect" (Lin et al.
  2022). Together they give 22.6 m3 an hour, 13 air changes. The grower
  rule of thumb of 4 to 6 air changes of a Martha tent would leave its CO2
  near 5,000 ppm at that rate, while the growers'
  humidifier sizing (3 to 6 L a day) matches the CO2 figure. Each rack
  gets a `tent_humidifier` on its bottom shelf (AC Infinity CLOUDFORGE T3,
  240 ml/h, 24 W), the whole-room T7 is no longer placed, and the
  commons rack (which had none) gets one too. THE BALANCE, every tent at
  90.0%: family seven tents 13.3 L and 1.3 kWh a day (was 27.5 L and 2.1 kWh
  for six racks), the mushroom room around them at about 62%; solo two tents
  5.6 L and 0.56 kWh a day (was 31 L and 2.4 kWh). The solo Energy loop
  CLOSES again (about 4.6 against 5.76 kWh); the family loop still does not
  (about 14.0 against 12.2, the exhaust fan's 0.7 kWh alone tipped it; two
  more panels would close it). Cisterns now reach 32 and 93 days. Left: the
  tents' CO2 is not tracked, so each tent draws the home's 400 ppm air. Six
  racks put about 148 g of CO2 an hour into the mushroom room, like four
  resting people, which its leakage dilutes by about 540 ppm. A real room this
  full needs its air moved for CO2, which the model does not yet do. The tent
  is drawn (2026-09-27, clear sheeting around open shelving), and the T3's own 4.5 L reservoir is not
  modelled.
- **Mushroom yields are sourced, and every harvest counts only what is eaten:
  DONE 2026-09-27.** A mushroom's "plant" is now the unit growers count by:
  one 5 lb fruiting block for oyster and shiitake, one square foot of cased
  compost bed for button (`data/garden/yields.ron`, MUSHROOMS). Oyster: about
  20% of the wet substrate over three or four flushes (Agrodok 40), 1 lb a
  block, first flush 25 days after spawning (Penn State), four flushes a week
  apart. Shiitake: 75 to 125% biological efficiency on supplemented sawdust
  (Penn State), 0.68 to 1.13 kg a block, first flush at 56 days, three flushes
  18 days apart. Button: USDA NASS's 5.10 to 5.89 lb per square foot a crop,
  first flush at 36 days, five breaks 8 days apart. The rack is now five
  shelves of two blocks, the ten its tent holds, so a block's
  `area_per_plant_m2` (0.36 m2) is the tent's loading, not the room a block
  needs (a Martha tent packs one into 0.12 to 0.22 m2). A rack went from about
  198 kcal a day, an unsourced placeholder, to 28. Harvest items now match
  the form of the crop: runner beans give green pods (a new
  `vegetable_runner_bean_0`, not dry beans), cinnamon dried quills, vanilla
  cured beans, cacao fermented dry beans (its yield per tree now sourced,
  0.8 to 1.0 kg), and saffron stays fresh stigmas with its calories on that
  basis. Peanut, sunflower, safflower, paddy rice, chestnut, hazelnut, walnut
  and coconut had kernel calories on items weighed with the shell or hull;
  each is now per 100 g as harvested (USDA's own refuse for the food row, or
  AFCM's peanut shelling percentage, IRRI's rice hull and Feedipedia's
  oil-type sunflower hull). THE TOTALS: the family home grows about 6,020 kcal
  a day (was 7,350) and its Food loop NO LONGER CLOSES (91% of 6,600); the
  one-person home about 1,780 (was 2,154), 81% of 2,200. Found, left alone:
  (1) three src tests follow the data and need a small change each:
  `farming::unit_tests` still lists oyster_mushroom as not sourced;
  `farming::soil`'s removal check (N against protein at a flat 6.25) reads
  1.45 for paddy rice and 1.43 for hulled sunflower now that both are on the
  harvest's basis; and the humidity tent save round-trip compares floats
  exactly, which the new 0.12 L of a block's water misses by one unit in the
  last place because serde_json parses floats best-effort unless its
  `float_roundtrip` feature is on. (2) The mushroom
  water column is still the old 0.3 L a shelf, spread so the tents' air is
  unchanged; sizing it from the harvest's water is left to the humidity
  model. (3) Oyster mushrooms need dim light to form caps (Agrodok, Penn
  State), which `needs_light` does not model, and the rack still reads "no
  light needed". (4) A harvest returns two spawn items as seed (FIXED
  2026-09-27: a fungus returns none, and its spawn is bought). (5) A perennial
  like cacao counts one harvest per `growth_days` (five years), not one a year.
  (6) data/home_outline.json and homestead-solo-design.md still plan 50 kcal
  a rack.
- **The ship's air and the garden's water are closed loops: DONE
  2026-09-26** (ship life support; the operator chose the spaceship first,
  2026-09-27; design and every balance in
  [ship-life-support.md](ship-life-support.md), every number in
  `data/life_support.ron` with its quoted, dated source). The home's own air
  was held at 40% for nothing and swallowed the greenhouse's 541 L a day; it
  now keeps its own vapour, carbon dioxide and oxygen (`HomeAirState`, saved
  in the soil memory) and so does every grow room and tent, solved exactly
  per tick (`life_support::relax`: the air's water ledger closes to the gram).
  WATER: a new `air_handler` machine, a ducted chilled-water fan coil
  (Carrier 42CT size 14: 1,842 m3/h, 325 W) whose coil leaves the air at the
  ISS condensing heat exchanger's 5.9 C dew point, holds a grow room at 75%
  (BVAD 2022's "about 75%" for plants) or the home's own air at 50% (the top
  of the EPA's ideal), and pipes its condensate back to the cistern through
  its plumbing island. The irrigation now also draws the water a crop's tissue
  keeps (BVAD 2022 Table 4-90: 2 to 10% by crop). THE CLOCKS (BUG-092 item 7,
  not resolved): the air runs on game hours, the tanks on real minutes, and
  every flow crosses as litres a day on both sides from one physical state, so
  a day's water balances on each clock; a test proves it (drawn 108.79 L =
  returned 102.39 + kept 6.40 + lost 0.0001 a day, the same at 1x and 2x).
  Found on the way and fixed: the tanks' f32 level dropped every per-frame
  change under half a float step, so at 60 frames a second a household tap's
  0.17 L/min never left the cistern; the rounding is now carried. AIR: people
  breathe in proportion to their food (Hanford 2004's crew member, scaled the
  way data/home_outline.json does); crops take up carbon dioxide and give out
  oxygen at NASA's measured grams per litre they breathe out (BVAD 2022 Table
  4-91), in their lit hours, less in thin air (Kimball 1983's 33% per
  doubling); the mushroom tents breathe out their substrate's CO2; the 25 W
  "air recycler" that made oxygen is now the ISS CDRA scrubber (4.74 kg a day,
  860 W), idle below 2,636 ppm; the air leaks 0.02 kg a day per room (BVAD).
  The old stand-in that suffocated the household minutes after a power cut is
  gone. A Settings > Gameplay "Ship life support" switch (Station-supplied by
  default, or Realistic) decides who powers the air machines; nothing else
  changes between the modes. THE BALANCE at full planting: the family home's
  seven handlers (greenhouse 2, court 1, home 4) return all 666 L a day put in
  the air, the garden's net draw is 91 L a day (tissue 40, outdoor fields 57,
  less the household's breath), and the crops hold the home's carbon dioxide
  at 440 to 500 ppm, rising toward about 800; the solo home's five return all
  250 L, net draw 43 L. Both Water loops close; the cistern reaches about 24
  and 65 days. FOUND, written into the homes' loops: (1) the Energy loops no
  longer close in the Realistic mode: the handlers average about 1.5 kW (36
  kWh a day) and 1.0 kW (24 kWh), because the greenhouse and the court leak
  about 215 L a day into the drier home air, where condensing costs four times
  the fan power a litre it does in the greenhouse; a home at 60% halves it
  (measured), and tighter bulkheads or EC fans would cut it too. (2) The Air
  loops do not quite close: the crops give out one oxygen per carbon dioxide
  they fix, but people burning fat and protein use more (respiratory quotient
  0.87), so the homes lose about 0.4 and 0.2 kg of oxygen a day, years before
  it matters; no electrolyser is placed. (3) The mushroom tents run at 1,200
  to 1,640 ppm, over the 1,000 their oysters fruit under, because their room
  sits about 540 ppm over the home's, as this doc estimated; no fan answers
  CO2. (4) The court's towers breathe out 106 L a day and had no air handler:
  its air saturated and 45 L a day condensed on its walls, until one was
  placed. Left: the latent heat (about 19 and 7 kW) and the Climate loop,
  trace contaminants, the station's well and rain, and the "outdoor" fields
  that breathe into a sky the station does not have.
- **Rice is milled, sunflower is pressed, mushrooms do not seed themselves,
  and a charcoal fire's ash limes the garden: DONE 2026-09-27.** Five gaps
  from the 2026-09-27 review; every number and its quote sit beside it in
  `data/recipes.csv`, `data/items.csv`, `data/food/crop_nutrition.ron` and
  `data/garden/soil_ph.ron`. (1) RICE: the garden harvests paddy, still in
  its hull (`grain_rice_0`, now "Paddy Rice", and no longer food as it is),
  and a new `mill_rice` at the grain mill turns 10 paddy (5 kg) into 7 white
  rice, 1 kg of hulls and 0.5 kg of bran, IRRI's "20% husk, 8−12% bran ...
  and 68−72% milled rice" taken at its ideal. The porridge now simmers milled
  rice (it took the paddy). The hulls (40% carbon, 0.8% N, IRRI) and the bran
  (14.8% protein, Feedipedia) compost together, 3 hulls and 2 bran with 4 L
  of water at C:N 34 and 56% water, into 2 bags (`compost_rice_hulls`). The
  vendor's rice is a 500 g bag of white rice (its trade entry said 0.2 kg of
  "hulled grain"). (2) SUNFLOWER: `press_oil_sunflower` presses 6 whole
  oil-type seed (3 kg, 44.5% oil, Feedipedia) into 2 oil and 4 cake that
  keeps 17% oil, inside Feedipedia's "15-20%" for a screw press. (3)
  MUSHROOM SEED: a fungus's harvest hands back no spawn
  (`picking::harvest_returns_seed`, read off plants.csv `needs_light`
  false), because spawn is grain sterilised and grown from a stored culture
  (Penn State), for which the game has no equipment; the vendor now sells
  shiitake and button spawn beside the oyster spawn. (4) HULLED GRAINS:
  barley's harvest is threshed covered barley, hull on, while its calories
  were the dehulled grain's; it is now per 100 g as harvested, FDC "Barley,
  hulled" x 0.87 (the husk "accounts for an average of 13%", Lukinac and
  Jukić 2022), 308 kcal, not 354. Spelt was checked and agrees with itself:
  its item is dehulled kernels, as its FDC row is. (5) WOOD ASH: a new
  `wood_ash_0` (39 g, FAO's "about 3%" ash of a 1.3 kg charcoal lump) is
  left by the eight kiln, forge and smelter recipes that burn charcoal as
  fuel, one per lump; the six ore smelts leave none, because their charcoal
  is charged with the ore and its ash goes into the slag. The Garden panel
  gains a Wood ash button: 50% calcium carbonate equivalent (Iowa State's
  figure for an untested ash; Wisconsin and UGA agree), capped at the
  guides' 20 lb per 1,000 sq ft a year (97.6 g a m2, 0.128 of a pH unit in
  the loam), with a 15-day half-life (a game estimate; UNH and Iowa State
  say only "more quickly than lime"), and its 3% potash (the 0-1-3 analysis
  Iowa State and UGA give) goes straight into the unit's K2O store.
  `tests/byproduct_use_lint.rs` now counts a soil amendment as a use of a
  byproduct, and holds the ash rule, IRRI's split and the sunflower press's
  oil balance. Fixed on the way: `fire_brick` made 12 kg of brick out of 7.2
  kg of wet clay; it takes 8 clay (14.4 kg) now, the rest leaving as water.
  Found, left alone: (1) there is no cooked-rice item (the porridge is the
  only rice dish); one needs a nutrition profile in data/food_system.ron.
  (2) Barley is still eaten hull and all from the pack, and spelt's harvest
  skips the dehuller; both want a dehulling or pearling step like rice's.
  (3) Oats likewise: `grain_oat_0` is groats with groats' calories, while its
  removal columns weigh whole oats, hull on. (4) The alien fungi's spores
  are sold nowhere, like every alien plant's seed, and no harvest returns
  them now either. (5) `make_charcoal` burns part of its wood to char the
  rest and leaves no ash; a fire of plain wood is not modelled. (6) The ash
  cap is per application, like sulfur's; the guides' once a year would need
  a record of each unit's last dose. (7) The ash's 1% P2O5 is not credited,
  and a crop that has not ticked yet gets no potash. (8) Of the ore smelts
  whose ash goes to the slag, only iron and copper leave any slag.

## Defects found (things that are wrong, not merely missing)

1. **Items vanish when the backpack is full.** "Take to backpack" removes the
   placed item before adding it, and ignores the overflow
   (`src/gui/pages/inventory.rs` ~1796; `InventoryOp::Add` return value
   ignored at `src/systems/inventory/mod.rs` ~764). Manual craft outputs that
   do not fit are also discarded with only a log line
   (`src/systems/crafting/mod.rs` ~518-524).
2. **The showcase garden is invisible to the Garden panel and the sliders.**
   `auto_seed_showcase` tags crops with machine INSTANCE ids (`ntower_0`,
   `grain_field_1`), the Garden panel and the water/nutrient sliders key on
   type or tower-config ids, and the field climate check keys on a `_field`
   suffix the instance ids do not have (`src/engine/ipc.rs` ~222,
   `src/gui/pages/inventory.rs` ~212-247 and ~1894-1913,
   `src/systems/farming/mod.rs` ~1132). Its `grain_tray` and `potato_bed`
   keys match no machine type, so those surfaces are never seeded
   (`data/world/showcase.ron`).
3. **Twelve recipe inputs have no source at all**, blocking 12 recipes, mostly
   name mismatches: `painkiller_0`/`antibiotic_0` vs the made
   `painkillers_0`/`antibiotics_0`, `charcoal_lump_0` vs `coal_0`,
   `fiberglass` not an item, `animal_fat_0` only from animals that never spawn,
   `salt_block_0` unsourced, and cotton/hemp/jute/apple/banana/coffee with no
   seed item. Three recipe ids are duplicated (`craft_compass`,
   `craft_binoculars`, `refine_fuel`).
4. **The first seed only comes from a dev button.** Harvest returns two seeds,
   but `seed_<id>_0` exists for 61 of 189 plants, the vendor's `seed_bag_0` is
   not an item, and `starting_items` in `data/player.toml` is never read (the
   player spawns empty).
5. **Cooked foods feed nobody.** Cake, pie, omelette, stew, sandwich, juice
   and nine more have no nutrition profile, so eating them does nothing
   (`src/systems/food.rs` ~191-238), while `grain_mill_0` and `grain_silo_0`
   count as food because of their `grain_` prefix.
6. **Crafting XP is skipped** when a machine's own vessel takes every output
   (early return, `src/systems/crafting/mod.rs` ~1281).
7. Six of the Crafting page's eleven sidebar categories match no recipe
   (`data/crafting/categories.json`), so 225 recipes appear only under "All".

## Missing, by area

**Water.** Irrigation takes no water from the tanks: farming only reads
whether the cistern is above 2%. The Irrigation machine draws from plumbing
but farming never reads it, so switching it off changes nothing. Rain waters
no crop. Each plant's `water_liters_per_day` is display-only. Tank litres and
water items (jugs) do not convert in either direction.

**Gardening.** One nutrient slider instead of N-P-K (the N-P-K, pH and
humidity columns in `plants.csv` are display-only; `farming/soil.rs` is an
unused scaffold). No pests, disease, weeds, pollination or rotation. Grow
lights and day length do nothing. Yield ignores the crop's health. Legumes and
fiber never spoil; nothing consumes `grain_wheat_0`. **Plant models: 12 of 189
species** have 3D growth-stage models (tomato, carrot, wheat, lettuce, corn,
rice, pumpkin, beet, apple, orange, watermelon, bamboo); six more model sets
(mushroom, berry bush, flowers, cactus, palm, grass) match no plant id; towers
always draw the procedural stand-in.

**Crafting.** No tools-required column, tool wear, quality tiers,
byproducts, failure, recipe discovery or station power draw
(`quality_levels` in `manufacturing.ron` is parsed and unused). 256 of 308
outputs are used by nothing. In the one-person home, 12 of 17 station types
are unreachable (160 recipes); the construction editor can add any machine
for free in any play mode.

**Containers.** The design is half there: 17 container types and 12 content
classes with a `food_safe` flag, and a written rule that a food container
"must not have previously held a non-food-safe class"
(`data/containers/content_classes.ron`). None of that rule is code: a
container forgets its contents the moment it is emptied, `base_material` and
`food_safe` are never read, there is no lining, no cleaning, no residue. The
17 types are not items (the items called crate, barrel, fuel drum and water
tank are plain solids with no capacity). Liquids are counted as whole jugs in
slots, not litres in a tank.

**Physical storage.** Nothing stored is drawn in the world: no crates, piles,
tank levels or cargo that fills up. The organize layer is a flat list of
labels with a path string and no capacity; only a walk-up card shows
"X / Y L". The construction editor already has `cargo` and `storage` zone
types whose filler tiles boxes across the floor at a FIXED amount
(`data/blueprints/zone_filler.ron`, `home_structure::generate_zone_filler`):
that seam is where a Starmade-style "the room fills as the stock grows" would
go.

## Proposed order (not yet decided by the operator)

1. **Defects 1-7.** Small, and each one is a place the game lies to the
   player or loses their things.
2. **Water is real.** Irrigation draws litres from the cistern at each plant's
   `water_liters_per_day`, only while the Irrigation machine is powered; rain
   waters field crops; fill a jug from a tank and pour it back.
3. **Containers are real.** The 17 types become items with capacity; each
   container carries its material and lining and a content history. A
   petroleum, solvent, pesticide or cleaner history bars food and drinking
   water permanently; food-to-food switches need a cleaning action (for
   dairy: rinse, hot alkaline wash, acid rinse, sanitise); linings and
   plastics remember previous contents for longer than stainless steel
   (Codex: 1, 2 or 3 fills). Reactive metals refuse acidic or salty foods.
   All of it from the findings table, as data. Fluids become litres in a
   tank.
4. **You can see your stuff.** Storage and cargo zones fill with crates,
   sacks and barrels in proportion to what is stored; tanks show their level.
5. **Crafting depth**: tools required and worn, byproducts, station power,
   quality. **Gardening depth**: N-P-K, pests, light, yield from health, the
   six unmapped plant models, then more species.
