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
  switch shipped in the same release. Still to do: whitefly and thrips, row
  covers, and pests during the offline catch-up.
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
  skipped because burning gives the player no ash item yet; crop removal of
  calcium and magnesium, and legumes, which also acidify (NRCS), are not
  counted; hand-planted crops have no unit, so they sit at 6.5 and cannot be
  limed; pH does not step during the offline catch-up; a tower's solution
  cannot yet be set per crop (a blueberry wants a lower one), and the Home
  page's tower compatibility check still intersects the plants.csv SOIL
  windows for a reservoir the pH model treats as held.
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
  crop. Not modelled yet, and said on the hive's card: a colony lasts 10 to
  14 weeks, and LSU warns a small garden's few flowers can leave the bees
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
