# The starter survival garden: the fewest crops, the least space, everything plants can give

Status: a DESIGN, written 2026-11-06, for the fresh mothership start in
[mothership-master-plan.md](mothership-master-plan.md). Nothing in the game reads it yet. The
numbers are computed, not typed: `node scripts/survival-garden.js` works them out from the game's
own crop files and three new data files, and rewrites the block marked "generated" below
(`--write`; `--check` says whether this page still matches the data). Every operator decision is
quoted; everything else is a **Proposal**.

The operator, 2026-11-06, on wiping today's garden (1,575 of its 1,976 crops are dead): "Our goal
is a minimalist survival garden that provides all the nutrients it possibly can."

## 0. In short

- **One resident, bare minimum: four crops on 135 m2 (1,453 sq ft) of lit beds**: **soybean 62
  plots, broccoli 36, carrot 19, potato 18** (1 m2 plots, two to a 2 x 1 m bed). That is 176 m2
  (1,889 sq ft) of floor at NASA's path allowance, or 242 m2 (2,604 sq ft) laid out in the
  homestead's greenhouse (section 7). They feed 2,101 kcal and 120 g of protein a
  day, every essential amino acid at 2.8 times its need or more, both essential fats, and every
  vitamin and mineral USDA measures in crops at or above the US Dietary Reference Intake except
  sodium (salt, below), with the plant-diet allowances the DRI report itself asks for (iron 1.8
  times, zinc 1.5 times).
- **Three crops cannot do it, under the caps below. Four can.** The four are not the only answer:
  seven crops would save 6 percent (126 m2), and the near-ties are listed in the generated block.
- **What no garden gives, and how the ship covers it.** Vitamin B12 (2.4 ug a day: a supplement
  or fortified food from the ship's fermentation plant; fish from the aquaponic tank is the home
  option). Vitamin D (15 ug a day: one shelf of the mushroom rack, its button mushrooms flashed
  under an ultraviolet lamp; or a supplement). Salt (3.9 g a day for the sodium and chloride) and
  the iodine in it (iodized at 49 mg per kg, WHO's 2014 figure for 4 g of salt a day, covers the
  150 ug). Selenium is the bed's, not the crop's: selenate in the fertiliser, as Finland has done
  since 1985. Biotin, chromium, molybdenum and fluoride are not measured by USDA in these foods,
  so this design claims nothing about them.
- **Nutrition is half again the area of calories.** Energy and protein alone need 87 m2; the
  vitamins, minerals and fats bring it to 134 m2. The most expensive targets are energy, zinc and
  vitamin E (choline in the comfort tier).
- **Comfort with current technology** (hydroponic yields NASA measured, fat inside the DRI range,
  no food over half the day's energy, a plot or two of every flavour crop): **13 crops on 94 m2
  (1,012 sq ft)**, 2,672 kcal. The hydroponic potato is 2.6 times the field potato, but the
  soybean, which carries the zinc, choline and fats, only 1.5 times, so the micronutrient-complete
  garden stays near 90 m2, well above NASA's 40 to 50 m2 a person, which counts energy and air,
  not every DRI.
- **Light is the price.** Between the stars the sunline is lamps: 2.78 kWh per m2 a day at 28 mol
  and 2.8 umol/J, so the bare garden's light is 375 kWh a day (15.6 kW) and the comfort garden's
  261 kWh (10.9 kW), inside the closed-loop test's 3.5 to 16 kW a person for lighting crops.
- **It scales almost exactly with people** (about 134.5 m2 a resident at 2, 4 and 8); what does not is
  rounding to whole plots, the mushroom shelf (one serves about one and a half people), the seed
  stock and the nursery, and the labour, which batches.
- **Findings that do not balance**, section 9: plants.csv's dry bean runs on a snap bean's 60 days;
  BVAD's sweet potato row disagrees with itself; the game's tomato, pepper and cucumber deliver a
  high tunnel's season in a field's window; the aquaponic tank's "B12 + omega-3" closes about half
  the B12 at its catalogued yield; the carrot's own humidity range sits below the greenhouse's air.

## 1. Assumptions (each moves the answer; each is in the data, so each can be changed and re-run)

| Assumption | Value | Where it lives, and why |
|---|---|---|
| The person | bare 2,100 kcal and 57 g protein; comfort 2,600 kcal and 75 g | data/closed_loop/person.ron, the same person the closed-loop test sizes |
| Vitamins and minerals | the HIGHER of the male and female DRI for ages 19 to 50, because the garden feeds whoever lives there | data/food/nutrient_requirements.json, read from the National Academies' 2019 DRI summary tables |
| Plant-diet allowances | iron x1.8, zinc x1.5 | the DRI report's own words (quoted in section 14) |
| Amino acids | FAO's adult scoring pattern times the protein target | FAO Food and Nutrition Paper 92, Table 3 |
| Light | the sunline: 12 h a day at 28 mol of photons per m2 (648 umol/m2/s) | NASA grew potato, sweet potato, soybean and peanut in exactly this (BVAD 2022 Table 4-89), and it is the game's 06:00 to 18:00 solar dial |
| Bare yields | the game's own: data/plants.csv yields, spacing and days, data/garden/yields.ron sources, harvest_windows.ron picking windows | a soil bed managed like a good field, replanted all year because the sunline has no seasons |
| Comfort yields | BVAD 2022 Table 4-90 hydroponic yields where NASA measured the crop at 28 mol or less; field yields otherwise | wheat (115 mol) and rice (33 mol) need more light than the sunline and keep field yields |
| A plot | 1 m2, holding floor(1 / area per plant) plants | data/garden/grow_media.ron: a 2 x 1 m bed is two 1 m2 plots |
| Paths | floor = canopy x 1.3 | closed-loop-test.md: 50 m2 of canopy needs 65 m2 of floor |
| Edible share | USDA's refuse (SR28) taken off the harvest; the shell or hull share crop_nutrition.ron already uses; potato refuse 5 percent, not USDA's 25 (the row is flesh AND skin) | data/food/crop_micronutrients.json |
| Cooking | USDA retention factors: potatoes boiled in their skins, soybeans boiled two hours in a stew, greens boiled with the water used, grains as porridge, carrots and salads raw | the same file, section `retention` |
| Seed kept back, storage loss | per crop, mostly ESTIMATES (potato 3.9 percent derived from UMN's seed piece) | data/garden/survival_garden.json |
| Food eaten | at most 3.0 kg (6.6 lb) a day and 1.0 kg of any one crop (bare); 2.5 kg and no food over half the energy (comfort); eaten energy within 10 percent of the target | ESTIMATES for a diet a person would keep eating; the run without them is printed |
| ULs | held at 97 percent of the UL where the UL counts food (manganese, zinc, copper, calcium, phosphorus, vitamin C, B6, choline, selenium); iron's UL reported, not held | the DRI says of iron: "the risk of adverse effects from dietary sources appears to be low" |
| Choosing the bare garden | the fewest crops that meet every target, and of those the least area | the operator's "smallest set of crops ... least space" |

## 2. Method

A linear program. Each candidate crop c gets an area x_c in m2 of lit canopy. One square metre of
a crop yields, each day, (plants a plot x harvest a plant) / (days in the bed), less refuse, seed
and storage losses; every gram eaten carries USDA's nutrients less what cooking takes. Minimise the
total area so that, for every nutrient, the sum over crops of (area x nutrient a square metre-day)
is at least the target; and, where a ceiling applies (a UL, the energy ceiling, the caps on food
eaten), at most the ceiling. Then:

1. **The fewest crops.** Every set of k crops is tried for k = 1, 2, 3, ... while the number of
   sets stays under 60,000 (so the smallest feasible k is proven, not guessed), with a cheap test
   that throws out a set first if even all of its energy in its most nutrient-dense crop could not
   reach a target.
2. **Whole plots.** The areas are rounded up to whole 1 m2 plots (every minimum then holds), then
   plots are taken back one at a time while every minimum still holds, first where a ceiling is
   exceeded. Minimums are never traded away.
3. **Sensitivity.** The report prints what 10 percent off each binding target would save, the
   garden for energy and protein alone, and the garden without the caps on food eaten.

The simplex is the textbook two-phase method with Bland's rule, in plain node, no packages
(`scripts/survival-garden.js`). Re-run it when a crop file changes: `node
scripts/survival-garden.js --write`, then read the diff.

## 3. The garden (generated)

<!-- survival-garden.js:begin (generated: node scripts/survival-garden.js --write) -->

### Tier: Bare minimum: soil beds under the sunline, field yields, the fewest crops

- Smallest area, any crops (the linear program over all 30 candidates): **126.4 m2 (1,361 sq ft)** of canopy, 7 crops.
- Smallest area with exactly k crops, k = 1: none; 2: none; 3: none; 4: 134.3 m2 (1,446 sq ft) (Potato, Soybean, Broccoli, Carrot); 5: 130.5 m2 (1,404 sq ft) (Potato, Soybean, Sunflower, Carrot, Lettuce); 6: 127.2 m2 (1,369 sq ft) (Potato, Soybean, Sunflower, Carrot, Lettuce, Broccoli).
- Energy and protein alone (the way a calorie plan sizes a farm, every other nutrient ignored): 87.1 m2 (938 sq ft) (Potato 28.3, Soybean 6.4, Beet 17.7, Parsnip 30.0, Rice 4.7). The rest of the area is what the vitamins, minerals and fats cost.
- Without the caps on food eaten (3.0 kg, 6.6 lb, a day, and 1.0 kg of any one crop), the smallest garden would be 111.7 m2 (1,202 sq ft) (Potato 12.6, Soybean 44.3, Carrot 31.9, Beet 5.6, Lettuce 17.3), feeding 4.2 kg (9.3 lb) of food a day.
- The closest 4-crop gardens (near-ties: a few percent is inside the uncertainty of the yields and the estimates): Potato + Soybean + Broccoli + Carrot 134.3; Potato + Soybean + Sunflower + Lettuce 144.0; Soybean + Sunflower + Cauliflower + Carrot 150.2; Soybean + Broccoli + Carrot + Onion 150.8; Soybean + Sunflower + Broccoli + Carrot 152.6; Sweet Potato + Soybean + Broccoli + Carrot 153.9 m2.
- What each binding target costs (canopy saved if that one target were 10 percent lower): Energy 12.4 m2, Zinc 7.9 m2, Vitamin E 9.8 m2.
- The garden, rounded up to whole 1 m2 plots: **135.0 m2 (1,453 sq ft) of lit canopy**, 135 plots in 68 beds of 2 m2; with paths (x1.3) **175.5 m2 (1,889 sq ft)** of floor.
- Light, if the sunline is lamps: 375.0 kWh a day (2.78 kWh per m2 at 28 mol and 2.8 umol/J, photons at the canopy only).

| Crop | Plots (m2) | Plants a plot | Days in bed | Sow a plot every | Harvest a day | Eaten a day | kcal a day | Protein | Water to air a day | Yield source |
|---|---|---|---|---|---|---|---|---|---|---|
| Soybean | 62 | 24 | 100 | 1.6 days | 205 g (0.45 lb) | 200 g | 893 | 73.0 g | 162.2 L (42.85 gal) | plants.csv / yields.ron |
| Broccoli | 36 | 5 | 65 | 1.8 days | 1,445 g (3.18 lb) | 855 g | 291 | 24.1 g | 84.1 L (22.21 gal) | plants.csv / yields.ron |
| Carrot | 19 | 64 | 75 | 3.9 days | 1,219 g (2.69 lb) | 1,042 g | 427 | 9.7 g | 53.5 L (14.13 gal) | plants.csv / yields.ron |
| Potato | 18 | 4 | 90 | 5.0 days | 719 g (1.58 lb) | 636 g | 490 | 13.0 g | 44.9 L (11.87 gal) | plants.csv / yields.ron |
| **Total** | 135 | | | | 3,588 g (7.91 lb) | 2,734 g | 2,101 | 119.9 g | 344.7 L (91.06 gal) | |

| Nutrient | Daily target | Supplied | Percent | Gap or margin | Notes |
|---|---|---|---|---|---|
| Energy | 2,100 to 2,310 kcal | 2,101 kcal | 100% | +0% |  |
| Protein | 57.0 g | 120 g | 210% | +110% |  |
| histidine | 0.86 g | 3.34 g | 391% | +291% | survives 85% digestibility |
| isoleucine | 1.71 g | 5.84 g | 342% | +242% | survives 85% digestibility |
| leucine | 3.36 g | 9.41 g | 280% | +180% | survives 85% digestibility |
| lysine | 2.57 g | 8.30 g | 324% | +224% | survives 85% digestibility |
| sulphur amino acids | 1.25 g | 4.40 g | 351% | +251% | survives 85% digestibility |
| aromatic amino acids | 2.17 g | 10.7 g | 492% | +392% | survives 85% digestibility |
| threonine | 1.31 g | 6.70 g | 511% | +411% | survives 85% digestibility |
| tryptophan | 0.34 g | 1.72 g | 504% | +404% | survives 85% digestibility |
| valine | 2.22 g | 6.50 g | 293% | +193% | survives 85% digestibility |
| Fibre | 38.0 g | 83.4 g | 219% | +119% |  |
| Carbohydrate | 130 g | 328 g | 253% | +153% |  |
| Linoleic acid (omega-6) | 17.0 g | 21.5 g | 127% | +27% |  |
| Alpha-linolenic acid (omega-3) | 1.60 g | 3.29 g | 205% | +105% |  |
| Calcium | 1,000 mg | 1,317 mg | 132% | +32% |  |
| Iron | 32.4 mg | 39.4 mg | 122% | +22% |  |
| Magnesium | 420 mg | 864 mg | 206% | +106% |  |
| Phosphorus | 700 mg | 2,471 mg | 353% | +253% |  |
| Potassium | 3,400 mg | 10,989 mg | 323% | +223% |  |
| Zinc | 16.5 mg | 16.6 mg | 101% | +1% |  |
| Copper | 0.90 mg | 3.54 mg | 394% | +294% |  |
| Manganese | 2.30 mg | 9.30 mg | 404% | +304% |  |
| Selenium | 55.0 ug | 60.6 ug | 110% | met at USDA values | not held: the bed's medium decides it (USDA's value is US farm soil); see the gaps |
| Sodium | 1,500 mg | 1,041 mg | 69% | 459 mg short | plants cannot: see the gaps |
| Iodine | 150 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Chloride | 2.30 g | - g | 0% | unknown | USDA does not measure it in these foods |
| Chromium | 35.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Molybdenum | 45.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Fluoride | 4.00 mg | - mg | 0% | unknown | USDA does not measure it in these foods |
| Vitamin A | 900 ug RAE | 8,956 ug RAE | 995% | +895% |  |
| Vitamin C | 90.0 mg | 698 mg | 775% | +675% |  |
| Vitamin D | 15.0 ug | - ug | 0% | 15.0 ug short | plants cannot: see the gaps |
| Vitamin E | 15.0 mg | 15.3 mg | 102% | +2% |  |
| Vitamin K | 120 ug | 1,113 ug | 927% | +827% |  |
| Thiamin (B1) | 1.20 mg | 2.43 mg | 203% | +103% |  |
| Riboflavin (B2) | 1.30 mg | 3.14 mg | 242% | +142% |  |
| Niacin (B3) | 16.0 mg | 23.8 mg | 149% | +49% |  |
| Pantothenic acid (B5) | 5.00 mg | 11.2 mg | 224% | +124% |  |
| Vitamin B6 | 1.30 mg | 5.08 mg | 390% | +290% |  |
| Folate | 400 ug DFE | 951 ug DFE | 238% | +138% |  |
| Vitamin B12 | 2.40 ug | - ug | 0% | 2.40 ug short | plants cannot: see the gaps |
| Biotin | 30.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Choline | 550 mg | 561 mg | 102% | +2% |  |

Binding in the linear program (these set the garden's size; rounding up to whole plots adds the margins above): Energy, Zinc, Vitamin E, the cap on carrot eaten.
Share of energy (at 9, 4 and 4 kcal a gram): fat 19.8%, protein 22.8%, carbohydrate 62.5%; the DRI ranges are 20 to 35, 10 to 35 and 45 to 65.

Loops:

- Carried out in the harvest a year (plants.csv removal columns): N 8.6 kg (18.9 lb), P 0.98 kg (P2O5 2.24), K 6.1 kg (K2O 7.3).
- Eaten a year (USDA, the same garden): N 7.0 kg (protein / 6.25), P 0.90 kg, K 4.0 kg. What is eaten leaves the body again: data/closed_loop/person.ron puts one person's excreta at 4.65 kg N and 0.44 to 0.61 kg P a year.
- Oxygen: eating 2,101 kcal a day burns 0.62 kg of oxygen (person.ron, 0.0700 kg per MJ). The crops made at least that much building the food, and more for their stalks and leaves, which give it back to the air only when they are composted.
- Vitamin D from UV-flashed button mushrooms: 150 g a day at 10 ug/100 g; one 0.72 m2 shelf of the mushroom rack holds 7 square feet of bed and gives 223 g a day (76 days a crop), so **1 shelf** (67 percent of one).

### Tier: Comfort with current technology: hydroponic yields where NASA measured them, fat inside the AMDR, variety

- Smallest area, any crops (the linear program over all 33 candidates): **92.8 m2 (999 sq ft)** of canopy, 13 crops.
- Energy and protein alone (the way a calorie plan sizes a farm, every other nutrient ignored): 63.1 m2 (679 sq ft) (Potato 14.5, Carrot 1.0, Onion 1.0, Garlic 1.0, Lettuce 1.0, Tomato 2.0, Bell Pepper 1.0, Strawberry 2.0, Bean 20.9, Peanut 18.7). The rest of the area is what the vitamins, minerals and fats cost.
- Without the caps on food eaten (2.5 kg, 5.5 lb, a day), the smallest garden would be 68.3 m2 (735 sq ft) (Potato 17.5, Carrot 1.0, Onion 1.0, Garlic 1.0, Lettuce 18.0, Tomato 2.0, Bell Pepper 9.8, Strawberry 2.0, Peanut 16.0), feeding 5.6 kg (12.4 lb) of food a day.
- What each binding target costs (canopy saved if that one target were 10 percent lower): Energy 6.0 m2, Calcium 3.5 m2, Vitamin E 1.5 m2, Choline 12.4 m2.
- The garden, rounded up to whole 1 m2 plots: **94.0 m2 (1,012 sq ft) of lit canopy**, 94 plots in 47 beds of 2 m2; with paths (x1.3) **122.2 m2 (1,315 sq ft)** of floor.
- Light, if the sunline is lamps: 261.1 kWh a day (2.78 kWh per m2 at 28 mol and 2.8 umol/J, photons at the canopy only).

| Crop | Plots (m2) | Plants a plot | Days in bed | Sow a plot every | Harvest a day | Eaten a day | kcal a day | Protein | Water to air a day | Yield source |
|---|---|---|---|---|---|---|---|---|---|---|
| Soybean | 39 | - | 97 | 2.5 days | 197 g (0.43 lb) | 192 g | 855 | 69.9 g | 183.3 L (48.42 gal) | BVAD 2022 (hydroponic) |
| Peanut | 18 | - | 104 | 5.8 days | 107 g (0.24 lb) | 103 g | 584 | 26.6 g | 49.9 L (13.17 gal) | BVAD 2022 (hydroponic) |
| Potato | 13 | - | 132 | 10.2 days | 1,369 g (3.02 lb) | 1,212 g | 933 | 24.9 g | 52.0 L (13.74 gal) | BVAD 2022 (hydroponic) |
| Cauliflower | 13 | 5 | 75 | 5.8 days | 452 g (1.00 lb) | 171 g | 43 | 3.3 g | 30.4 L (8.02 gal) | plants.csv / yields.ron |
| Tomato | 2 | - | 85 | 42.5 days | 348 g (0.77 lb) | 306 g | 55 | 2.7 g | 5.5 L (1.46 gal) | BVAD 2022 (hydroponic) |
| Strawberry | 2 | - | 100 | 50.0 days | 156 g (0.34 lb) | 139 g | 45 | 0.9 g | 4.4 L (1.17 gal) | BVAD 2022 (hydroponic) |
| Kale | 1 | 5 | 139 (55 + 84 picking) | 139.0 days | 55 g (0.12 lb) | 38 g | 13 | 1.1 g | 4.5 L (1.19 gal) | plants.csv / yields.ron |
| Carrot | 1 | - | 75 | 75.0 days | 75 g (0.16 lb) | 64 g | 26 | 0.6 g | 1.8 L (0.47 gal) | BVAD 2022 (hydroponic) |
| Onion | 1 | - | 50 | 50.0 days | 82 g (0.18 lb) | 71 g | 28 | 0.8 g | 1.7 L (0.46 gal) | BVAD 2022 (hydroponic) |
| Garlic | 1 | 21 | 240 | 240.0 days | 2 g (0.01 lb) | 2 g | 3 | 0.1 g | 2.5 L (0.66 gal) | plants.csv / yields.ron |
| Lettuce | 1 | - | 28 | 28.0 days | 131 g (0.29 lb) | 120 g | 20 | 1.5 g | 2.1 L (0.55 gal) | BVAD 2022 (hydroponic) |
| Bell Pepper | 1 | - | 85 | 85.0 days | 149 g (0.33 lb) | 118 g | 31 | 1.2 g | 2.8 L (0.73 gal) | BVAD 2022 (hydroponic) |
| Bean | 1 | - | 85 | 85.0 days | 11 g (0.02 lb) | 11 g | 36 | 2.5 g | 2.5 L (0.67 gal) | BVAD 2022 (hydroponic) |
| **Total** | 94 | | | | 3,133 g (6.91 lb) | 2,547 g | 2,672 | 136.1 g | 343.4 L (90.72 gal) | |

| Nutrient | Daily target | Supplied | Percent | Gap or margin | Notes |
|---|---|---|---|---|---|
| Energy | 2,600 to 2,860 kcal | 2,672 kcal | 103% | +3% |  |
| Protein | 75.0 g | 136 g | 181% | +81% |  |
| histidine | 1.13 g | 3.57 g | 318% | +218% | survives 85% digestibility |
| isoleucine | 2.25 g | 6.03 g | 268% | +168% | survives 85% digestibility |
| leucine | 4.43 g | 10.1 g | 227% | +127% | survives 85% digestibility |
| lysine | 3.38 g | 8.38 g | 248% | +148% | survives 85% digestibility |
| sulphur amino acids | 1.65 g | 3.99 g | 242% | +142% | survives 85% digestibility |
| aromatic amino acids | 2.85 g | 12.0 g | 423% | +323% | survives 85% digestibility |
| threonine | 1.73 g | 5.74 g | 333% | +233% | survives 85% digestibility |
| tryptophan | 0.45 g | 1.80 g | 399% | +299% | survives 85% digestibility |
| valine | 2.93 g | 6.91 g | 236% | +136% | survives 85% digestibility |
| Fibre | 38.0 g | 74.2 g | 195% | +95% |  |
| Carbohydrate | 130 g | 350 g | 269% | +169% |  |
| Linoleic acid (omega-6) | 17.0 g | 36.2 g | 213% | +113% |  |
| Alpha-linolenic acid (omega-3) | 1.60 g | 3.18 g | 199% | +99% |  |
| Calcium | 1,000 mg | 1,001 mg | 100% | +0% |  |
| Iron | 32.4 mg | 43.6 mg | 135% | +35% |  |
| Magnesium | 420 mg | 989 mg | 235% | +135% |  |
| Phosphorus | 700 mg | 2,545 mg | 364% | +264% |  |
| Potassium | 3,400 mg | 10,331 mg | 304% | +204% |  |
| Zinc | 16.5 mg | 17.7 mg | 107% | +7% |  |
| Copper | 0.90 mg | 4.90 mg | 544% | +444% |  |
| Manganese | 2.30 mg | 10.8 mg | 470% | +370% |  |
| Selenium | 55.0 ug | 49.9 ug | 91% | 5.09 ug short | not held: the bed's medium decides it (USDA's value is US farm soil); see the gaps |
| Sodium | 1,500 mg | 244 mg | 16% | 1,256 mg short | plants cannot: see the gaps |
| Iodine | 150 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Chloride | 2.30 g | - g | 0% | unknown | USDA does not measure it in these foods |
| Chromium | 35.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Molybdenum | 45.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Fluoride | 4.00 mg | - mg | 0% | unknown | USDA does not measure it in these foods |
| Vitamin A | 900 ug RAE | 1,462 ug RAE | 162% | +62% |  |
| Vitamin C | 90.0 mg | 559 mg | 622% | +522% |  |
| Vitamin D | 15.0 ug | - ug | 0% | 15.0 ug short | plants cannot: see the gaps |
| Vitamin E | 15.0 mg | 15.3 mg | 102% | +2% |  |
| Vitamin K | 120 ug | 457 ug | 381% | +281% |  |
| Thiamin (B1) | 1.20 mg | 2.73 mg | 227% | +127% |  |
| Riboflavin (B2) | 1.30 mg | 2.41 mg | 185% | +85% |  |
| Niacin (B3) | 16.0 mg | 32.5 mg | 203% | +103% |  |
| Pantothenic acid (B5) | 5.00 mg | 9.55 mg | 191% | +91% |  |
| Vitamin B6 | 1.30 mg | 5.51 mg | 424% | +324% |  |
| Folate | 400 ug DFE | 1,097 ug DFE | 274% | +174% |  |
| Vitamin B12 | 2.40 ug | - ug | 0% | 2.40 ug short | plants cannot: see the gaps |
| Biotin | 30.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Choline | 550 mg | 556 mg | 101% | +1% |  |

Binding in the linear program (these set the garden's size; rounding up to whole plots adds the margins above): Energy, Calcium, Manganese (its ceiling), Vitamin E, Choline, the cap on food eaten.
Share of energy (at 9, 4 and 4 kcal a gram): fat 31.4%, protein 20.4%, carbohydrate 52.4%; the DRI ranges are 20 to 35, 10 to 35 and 45 to 65.

Loops:

- Carried out in the harvest a year (plants.csv removal columns): N 8.8 kg (19.4 lb), P 1.01 kg (P2O5 2.31), K 5.3 kg (K2O 6.3).
- Eaten a year (USDA, the same garden): N 7.9 kg (protein / 6.25), P 0.93 kg, K 3.8 kg. What is eaten leaves the body again: data/closed_loop/person.ron puts one person's excreta at 4.65 kg N and 0.44 to 0.61 kg P a year.
- Oxygen: eating 2,672 kcal a day burns 0.78 kg of oxygen (person.ron, 0.0700 kg per MJ). The crops made at least that much building the food, and more for their stalks and leaves, which give it back to the air only when they are composted.
- Oxygen made by the crops with a BVAD figure (Table 4-91, whole plant): 1.80 kg a day.
- Vitamin D from UV-flashed button mushrooms: 150 g a day at 10 ug/100 g; one 0.72 m2 shelf of the mushroom rack holds 7 square feet of bed and gives 223 g a day (76 days a crop), so **1 shelf** (67 percent of one).

### Tier: Bare minimum with the mushroom room: the same, plus mushrooms grown in the dark on racks

- Smallest area, any crops (the linear program over all 32 candidates): **102.3 m2 (1,101 sq ft)** of floor (canopy plus a fifth of the mushroom shelf), 7 crops.
- Energy and protein alone (the way a calorie plan sizes a farm, every other nutrient ignored): 82.8 m2 (891 sq ft) (Potato 28.3, Parsnip 30.0, Rice 23.9, Button Mushroom 3.0). The rest of the area is what the vitamins, minerals and fats cost.
- Without the caps on food eaten (3.0 kg, 6.6 lb, a day, and 1.0 kg of any one crop), the smallest garden would be 83.3 m2 (897 sq ft) (Potato 12.5, Soybean 38.9, Kale 1.9, Carrot 7.3, Parsnip 21.3, Button Mushroom 7.2), feeding 3.9 kg (8.7 lb) of food a day.
- What each binding target costs (canopy saved if that one target were 10 percent lower): Energy 6.7 m2, Calcium 0.3 m2, Vitamin E 1.5 m2, Choline 6.0 m2.
- The garden, rounded up to whole 1 m2 plots: **101.0 m2 (1,087 sq ft) of lit canopy**, 101 plots in 51 beds of 2 m2; with paths (x1.3) **131.3 m2 (1,413 sq ft)** of floor. Plus 6.0 m2 (65 sq ft) of mushroom shelf, 2 racks of 3.6 m2.
- Light, if the sunline is lamps: 280.6 kWh a day (2.78 kWh per m2 at 28 mol and 2.8 umol/J, photons at the canopy only).

| Crop | Plots (m2) | Plants a plot | Days in bed | Sow a plot every | Harvest a day | Eaten a day | kcal a day | Protein | Water to air a day | Yield source |
|---|---|---|---|---|---|---|---|---|---|---|
| Soybean | 58 | 24 | 100 | 1.7 days | 192 g (0.42 lb) | 187 g | 835 | 68.3 g | 151.7 L (40.08 gal) | plants.csv / yields.ron |
| Carrot | 16 | 64 | 75 | 4.7 days | 1,027 g (2.26 lb) | 878 g | 360 | 8.2 g | 45.1 L (11.90 gal) | plants.csv / yields.ron |
| Parsnip | 13 | 24 | 100 | 7.7 days | 531 g (1.17 lb) | 433 g | 325 | 5.2 g | 31.5 L (8.32 gal) | plants.csv / yields.ron |
| Potato | 12 | 4 | 90 | 7.5 days | 479 g (1.06 lb) | 424 g | 327 | 8.7 g | 30.0 L (7.91 gal) | plants.csv / yields.ron |
| Button Mushroom | 4 | 10 | 76 (36 + 40 picking) | 19.0 days | 1,312 g (2.89 lb) | 1,209 g | 266 | 37.4 g | 1.4 L (0.36 gal) | plants.csv / yields.ron |
| Sunflower | 2 | 5 | 100 | 50.0 days | 4 g (0.01 lb) | 3 g | 18 | 0.6 g | 4.5 L (1.18 gal) | plants.csv / yields.ron |
| Oyster Mushroom | 2 | 2 | 53 (25 + 28 picking) | 26.5 days | 34 g (0.08 lb) | 29 g | 10 | 1.0 g | 0.5 L (0.13 gal) | plants.csv / yields.ron |
| **Total** | 107 | | | | 3,579 g (7.89 lb) | 3,163 g | 2,140 | 129.3 g | 264.6 L (69.89 gal) | |

| Nutrient | Daily target | Supplied | Percent | Gap or margin | Notes |
|---|---|---|---|---|---|
| Energy | 2,100 to 2,310 kcal | 2,140 kcal | 102% | +2% |  |
| Protein | 57.0 g | 129 g | 227% | +127% |  |
| histidine | 0.86 g | 3.28 g | 384% | +284% | survives 85% digestibility |
| isoleucine | 1.71 g | 5.63 g | 329% | +229% | survives 85% digestibility |
| leucine | 3.36 g | 9.06 g | 269% | +169% | survives 85% digestibility |
| lysine | 2.57 g | 7.77 g | 303% | +203% | survives 85% digestibility |
| sulphur amino acids | 1.25 g | 3.96 g | 316% | +216% | survives 85% digestibility |
| aromatic amino acids | 2.17 g | 9.99 g | 461% | +361% | survives 85% digestibility |
| threonine | 1.31 g | 6.63 g | 506% | +406% | survives 85% digestibility |
| tryptophan | 0.34 g | 1.75 g | 511% | +411% | survives 85% digestibility |
| valine | 2.22 g | 7.74 g | 348% | +248% | survives 85% digestibility |
| Fibre | 38.0 g | 85.1 g | 224% | +124% |  |
| Carbohydrate | 130 g | 334 g | 257% | +157% |  |
| Linoleic acid (omega-6) | 17.0 g | 22.4 g | 132% | +32% |  |
| Alpha-linolenic acid (omega-3) | 1.60 g | 2.57 g | 160% | +60% |  |
| Calcium | 1,000 mg | 1,000 mg | 100% | +0% |  |
| Iron | 32.4 mg | 38.6 mg | 119% | +19% |  |
| Magnesium | 420 mg | 841 mg | 200% | +100% |  |
| Phosphorus | 700 mg | 3,059 mg | 437% | +337% |  |
| Potassium | 3,400 mg | 12,396 mg | 365% | +265% |  |
| Zinc | 16.5 mg | 20.8 mg | 126% | +26% |  |
| Copper | 0.90 mg | 7.19 mg | 799% | +699% |  |
| Manganese | 2.30 mg | 9.70 mg | 422% | +322% |  |
| Selenium | 55.0 ug | 158 ug | 288% | met at USDA values | not held: the bed's medium decides it (USDA's value is US farm soil); see the gaps |
| Sodium | 1,500 mg | 743 mg | 50% | 757 mg short | plants cannot: see the gaps |
| Iodine | 150 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Chloride | 2.30 g | - g | 0% | unknown | USDA does not measure it in these foods |
| Chromium | 35.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Molybdenum | 45.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Fluoride | 4.00 mg | - mg | 0% | unknown | USDA does not measure it in these foods |
| Vitamin A | 900 ug RAE | 7,331 ug RAE | 815% | +715% |  |
| Vitamin C | 90.0 mg | 203 mg | 226% | +126% |  |
| Vitamin D | 15.0 ug | 2.62 ug | 17% | 12.4 ug short | plants cannot: see the gaps |
| Vitamin E | 15.0 mg | 15.1 mg | 100% | +0% |  |
| Vitamin K | 120 ug | 310 ug | 258% | +158% |  |
| Thiamin (B1) | 1.20 mg | 3.00 mg | 250% | +150% |  |
| Riboflavin (B2) | 1.30 mg | 7.12 mg | 548% | +448% |  |
| Niacin (B3) | 16.0 mg | 62.9 mg | 393% | +293% |  |
| Pantothenic acid (B5) | 5.00 mg | 26.2 mg | 525% | +425% |  |
| Vitamin B6 | 1.30 mg | 4.50 mg | 346% | +246% |  |
| Folate | 400 ug DFE | 925 ug DFE | 231% | +131% |  |
| Vitamin B12 | 2.40 ug | 0.48 ug | 20% | 1.92 ug short | plants cannot: see the gaps |
| Biotin | 30.0 ug | - ug | 0% | unknown | USDA does not measure it in these foods |
| Choline | 550 mg | 570 mg | 104% | +4% |  |

Binding in the linear program (these set the garden's size; rounding up to whole plots adds the margins above): Energy, Calcium, Vitamin E, Choline, the cap on food eaten, the cap on button mushroom eaten.
Share of energy (at 9, 4 and 4 kcal a gram): fat 19.7%, protein 24.2%, carbohydrate 62.5%; the DRI ranges are 20 to 35, 10 to 35 and 45 to 65.

Loops:

- Carried out in the harvest a year (plants.csv removal columns): N 5.9 kg (13.1 lb), P 0.79 kg (P2O5 1.81), K 4.3 kg (K2O 5.2); no removal figure for oyster_mushroom, button_mushroom.
- Eaten a year (USDA, the same garden): N 7.6 kg (protein / 6.25), P 1.12 kg, K 4.5 kg. What is eaten leaves the body again: data/closed_loop/person.ron puts one person's excreta at 4.65 kg N and 0.44 to 0.61 kg P a year.
- Oxygen: eating 2,140 kcal a day burns 0.63 kg of oxygen (person.ron, 0.0700 kg per MJ). The crops made at least that much building the food, and more for their stalks and leaves, which give it back to the air only when they are composted.
- Vitamin D from UV-flashed button mushrooms: 150 g a day at 10 ug/100 g; one 0.72 m2 shelf of the mushroom rack holds 7 square feet of bed and gives 223 g a day (76 days a crop), so **1 shelf** (67 percent of one).

### Scaling, bare tier

| Residents | Lit canopy | Per resident | Plots | Vitamin D shelves |
|---|---|---|---|---|
| 1 | 135.0 m2 (1,453 sq ft) | 135.0 m2 (1,453 sq ft) | 135 | 1 |
| 2 | 269.0 m2 (2,895 sq ft) | 134.5 m2 (1,448 sq ft) | 269 | 2 |
| 4 | 538.0 m2 (5,791 sq ft) | 134.5 m2 (1,448 sq ft) | 538 | 3 |
| 8 | 1,075.0 m2 (11,571 sq ft) | 134.4 m2 (1,446 sq ft) | 1075 | 6 |

<!-- survival-garden.js:end -->

## 4. What plants cannot supply, and how the ship covers it

| Need a day | Why the garden cannot | The ship's answer (realistic, cited) | Which cannot-close category |
|---|---|---|---|
| Vitamin B12, 2.4 ug | Only bacteria make it: its biosynthesis is "confined to certain members of the prokaryotic world" (Martens et al. 2002). Spirulina is not a source: 83 percent of its active-looking cobamide is pseudovitamin B12 (Watanabe et al. 1999), and USDA lists it at 0 ug | A supplement or fortified food from the ship's fermentation plant ("commercially more than 10 t of B12 are produced each year from a number of bacterial species", Martens). Home option: 152 g of tilapia a day from the aquaponic tank (USDA 1.58 ug/100 g); five eggs a day is too many to be the plan | raw chemistry inputs |
| Vitamin D, 15 ug | No green plant makes it | One shelf (0.72 m2) of button mushrooms, flashed under UV after harvest: 150 g a day at 10 ug/100 g, the level Cardwell et al. 2018 say survives storage and cooking ("likely to remain above 10 ug/100 g fresh weight"); USDA's raw UV-exposed mushroom holds 26.2. Or a supplement. UV-B on skin from the sunline is NOT designed: no dose or skin-cancer trade-off was sourced | the UV lamp is electronics |
| Iodine, 150 ug | USDA does not measure it; a crop holds what its medium gives | Iodized salt: the 3.9 g of salt the sodium need asks for anyway, iodized at 49 mg per kg (WHO 2014, for 4 g of salt a day, allowing 30 percent losses and 92 percent bioavailability) | raw chemistry inputs |
| Sodium 1,500 mg and chloride 2.3 g | Crops carry some (the bare garden 1,041 mg of sodium), not enough | The same 3.9 g of table salt: 1,500 mg of sodium (USDA 38,758 mg/100 g) and 2.35 g of chloride | raw chemistry inputs |
| Selenium, 55 ug | Plants do not need it and take up what the medium holds; USDA's values are US farm soils | Selenate in the fertiliser: Finland has added it to all crop fertilisers since 1985, "Currently all crop fertilizers contain 15 mg Se/kg", and spring cereals' selenium rose "on average 15-fold" (Alfthan et al. 2015) | raw chemistry inputs |
| EPA and DHA | No DRI requires them; the garden gives their precursor (ALA, 3.3 g a day, twice the AI) | Optional DHA-rich microalgae oil: "the origin of EPA/DHA in aquatic ecosystems is algae" (Doughman et al. 2007); the body's own conversion is "limited in men and ... further transformation to DHA is very low" (Burdge and Calder 2005) | raw chemistry inputs |
| Biotin, chromium, molybdenum, fluoride | Not measured by USDA SR Legacy in these foods | Not designed for; not claimed either way | |

The salt has a loop of its own: it leaves the body in urine, and the urine feeds the garden, but
crops take up little sodium, so a closed bed slowly salts unless something removes it. That is not
quantified here. `data/self_sufficiency/cannot_close.ron` now says the plate's gaps out loud, in the
raw chemistry entry and the footer.

## 5. The growing calendar under the sunline

The drum has no seasons: the sunline is on from 06:00 to 18:00 every day at the same strength, so
every crop is sown in small batches all year and harvested in small batches all year. The bare
garden, one resident:

| Crop | Plots | Days in the bed | Sow each week | First harvest | Then | Keeps |
|---|---|---|---|---|---|---|
| Soybean | 62 | 100 | 4 to 5 plots | day 100 | 4 to 5 plots a week, dry beans | a year, dry |
| Broccoli | 36 | 65 from transplant (plants.csv counts broccoli from transplant) | 4 plots, set out from a nursery tray started 4 to 6 weeks before | day 65 after the first set-out | 4 plots a week | days, fresh |
| Carrot | 19 | 75 | 2 plots | day 75 | 2 plots a week | weeks, cold and damp |
| Potato | 18 | 90 | 1 to 2 plots | day 90 | 1 to 2 plots a week | weeks to months, cold and damp |

About twelve plots go in each week. The garden reaches its steady state on day 100, when the first
soybeans come in; until then the resident eats from the mess hall, which the fleet keeps unlimited
in early development (decided 2026-10-04). A year of dry soybeans for one person is 73 kg (161 lb)
and keeps; potatoes keep cold and damp (UMN's storage advice for potatoes: "32-40°F and 95%
relative humidity, such as in the refrigerator"); the broccoli is eaten as it comes.

Under a 12 h day:

- **Soybean** flowers in response to short days (its maturity groups are a ladder of critical day
  lengths, procedural-plants.md); BVAD grew it at 12 h for 97 days, so the sunline suits it.
- **Potato** is grown at 12 h by BVAD (132 days), and some cultivars are injured by continuous
  light (data/garden/lighting.ron), so 12 h of dark is a feature.
- **Broccoli and carrot**: BVAD lists carrot at 28 mol a day with no photoperiod and has no
  broccoli row; neither is grown here for flowers, so a 12 h day is assumed to suit them (not
  confirmed in this pass). Both are biennials, and saving their seed is thought to need a cold
  spell the drum does not have, so their seed comes from a cold room or the ship's seed bank (also
  not confirmed in this pass).
- **The four share a climate**: 18 to 22 C, pH 6.0 to 6.5, humidity 50 to 70 percent, by their
  own plants.csv ranges. The greenhouse runs at 21 C, inside all four; its air is held near 75
  percent (ship-life-support.md), above the carrot's 70 and at the top of the potato's 75.
  Section 9.

## 6. Water and nutrients: closing the loops with the homestead

**Water.** The bare garden breathes out about 345 L (91 gal) of water a day (FAO-56 crop
evapotranspiration at the game's reference 3 mm a day, plants.csv's per-plant figures). The home's
air handlers condense it back to the tanks: at about 27 Wh of fan a litre out of 75 percent air
(ship-life-support.md section 5) that is about 9.3 kWh a day, and the coils take 2.45 MJ a litre of
latent heat, about 9.8 kW on average, to the ship's radiators. Water leaves the garden for good only
in the food: about 2.2 L a day in 2.7 kg of fresh food, which the resident returns to the home's own
water loop as urine, sweat and breath. So the garden is a loop, not a draw: what it needs from
outside is what leaks.

**Air.** Eating 2,101 kcal burns 0.62 kg of oxygen (person.ron's 0.0700 kg per MJ). The crops
released at least that much building the food, and more for their stalks and leaves, which give it
back only as they are composted inside the same air. BVAD's whole-plant figures for the comfort
garden's crops come to 1.80 kg of oxygen a day.

**Nutrients.** The bare harvest carries out 8.6 kg (19 lb) of nitrogen, 0.98 kg of phosphorus and
6.1 kg of potassium a year (plants.csv removal columns). What is eaten (7.0 kg N, 0.90 kg P, 4.0 kg
K, USDA) leaves the body again: the resident is a high-protein eater (120 g a day), above the
Finnish figures person.ron carries (4.65 kg N). The rest of the harvest, the refuse and the trimmings,
goes straight to the compost with the stalks and leaves. Nitrogen has a second source: the soybean
fixes about 55 percent of its own (plants.csv `n_fixed_pct`, Salvagiotti et al. 2008).
**Phosphorus does not close.** No biological process makes it; whatever the compost and urine loop
loses must be bought: at BIOS-3's measured 20 percent average mineral retention (closed-loop-test.md)
that is about 0.8 kg of phosphorus a year, at 99 percent about 10 g. Same shape as
cannot_close.ron's raw chemistry entry.

**Residue into food.** Oyster mushrooms are grown on crop waste: "the primary ingredients used for
Pleurotus spp. production are chopped wheat straw ... or cottonseed hulls ... or mixtures of both"
(Royse, Penn State, 2003), and the spent substrate is compost. Whether this garden's own soybean
stalks, potato haulm and broccoli leaves serve as well was not confirmed in this pass. The variant in
section 3 shows what the mushroom room could save (101 m2 of lit canopy instead of 135, with the two
racks the home already has), but it leans on about a kilogram of mushrooms a day and on substrate
this design does not measure, so it is a variant, not the plan.

## 7. The layout on the fresh home plot

Rung 1 of the master plan gives every homestead a 55 x 89 m plot (1.21 acres) and keeps the
homestead design whole (mothership-master-plan.md section 6.2). Its greenhouse is x 10..55, z 51..73
(990 m2, 10,660 sq ft), its mushroom room x 0..10 beside it. **Proposal** (placement data is not
changed in this task):

```
 GREENHOUSE, x 10..55, z 51..73 (home-local metres, x east, z south), not to scale
 x=10                            27.3                                                 55
 +-------------------------------+-----------------------------------------------------+ z 51
 | 9 rows x 8 beds, 2 x 1 m      |  comfort crops, the variety towers, the apothecary  |
 | (72 slots: 68 beds + 4 for    |  tower, the orchard and whatever the player adds    |
 | the broccoli nursery and a    |                                                     |
 | spare)  0.1 m between bed     |                                                     |
 | ends, 0.6 m paths             |                                                     |
 +-------------------------------+                                                     | z 66
 |  aisle to the mushroom room and the compost                                        |
 +-------------------------------------------------------------------------------------+ z 73
 MUSHROOM ROOM x 0..10: one of the two racks' shelves is the vitamin D shelf (UV lamp over it)
```

As data, one line in the home's `arrays` (infinite-of-x), replacing today's potato, oilseed and grain
tray arrays and moving the nine variety towers east, which today stand at x 14 to 18, z 62 to 66:

```ron
(
    machine: "potato_grow_bed",
    room: "room-greenhouse",
    origin: (11.5, 0.0, 52.5),
    rows: 9,
    cols: 8,
    spacing: (2.1, 1.6),
    id_prefix: "sbed",
    zone: "home",
),
```

The block is 16.8 x 14.4 m (55 x 47 ft) at its pitch, 242 m2 (2,604 sq ft): a quarter of the
greenhouse. Its paths are wider than the closed-loop test's x1.3 allowance (2.1 x 1.6 m of floor
for each 2 m2 bed is x1.7 here, the price of 1 m beds with walkable paths), which the 990 m2 room
easily affords. The comfort garden is 47 beds, the same block three rows shorter.
`potato_grow_bed` grows any crop (grow_media.ron's soil bed); its name says otherwise (section 10).

## 8. The same garden in real life

The game teaches real growing (two-realities.md), so the same four crops on Earth:

- **Area.** 135 m2 of beds is a garden a household can work, but only with a growing season all
  year. Outdoors in a temperate climate a bed gives fewer crops a year than the three to four the
  sunline allows, so the same diet needs more beds or a greenhouse with winter light (an estimate:
  the outdoor case is not computed here). The sunline is the reason the drum's garden is this small.
- **Cook the soybeans.** "Raw soybeans have high levels of trypsin inhibitors" and "Cooking heat
  largely destroys the trypsin inhibitors in soybeans" (EXTOXNET, University of Idaho, 1997). Dry
  kidney beans, which the comfort garden grows, are more dangerous raw: soak at least five hours,
  then "Boil beans in fresh water for at least 30 minutes", and never in a slow cooker, which "likely
  won't get hot enough" (Kansas State University, 2017).
- **Cut the green off potatoes.** "Remove any green areas before cooking potatoes." "Do not consume
  potatoes with extensive green areas." "Potato sprouts can be toxic; you should not eat them."
  (UMN Extension, reviewed 2026.)
- **Saving carrot seed means knowing poison hemlock.** A carrot left to flower looks like its wild
  relatives, and "All parts of poison hemlock are toxic to humans and other animals"; it "resembles
  carrot and parsley", and its purple-spotted stem is "very distinctive" (Penn State Extension,
  2025). Buy carrot seed, or save it only from plants you sowed in a bed you know.
- **Peanuts** (comfort tier): dry them before storing; damp stored peanuts grow Aspergillus, whose
  aflatoxins cause "health hazards and even death" (Kumar et al. 2016). The storage moisture to aim
  for was not confirmed in this pass.
- **The gaps are the same on Earth.** Take B12 on a plant diet, use iodized salt, get vitamin D from
  sun, a supplement or UV-treated mushrooms. Those three are the real-life version of section 4.

## 9. What does not balance (findings)

1. **plants.csv's dry bean runs on a snap bean's clock.** `bean` harvests dry beans
   (items.csv `legume_bean_0`, NASS North Dakota's dry bean yield) in 60 days; NASA grows dry bean
   in 85 (BVAD Table 4-89). At 60 days the game's bean yields about 1.4 times a day what it would
   at 85. This design uses 85 days; proposal: plants.csv `bean` growth_days 85, or a sourced field
   figure for dry beans.
2. **BVAD's sweet potato row disagrees with itself.** Table 4-90 gives 24.7 g dry and 51.72 g fresh
   a m2 a day at 71 percent water; 24.7 g dry at 71 percent water is 85 g fresh. This design uses
   the lower figure. Anyone citing BVAD's sweet potato should say which.
3. **The game's tomato, pepper and cucumber deliver a high tunnel's season in a field's window**
   (harvest_windows.ron says so itself). Unchecked, the optimiser picked cucumber as a calorie crop:
   its daily yield per m2 beat the potato's. They are excluded from sizing here and grown only as
   flavour; a high-tunnel picking window would fix the data.
4. **The aquaponic tank "closes B12 + omega-3" only by half.** grow_media.ron's note and the solo
   home's catalogue stat say so; at the tank's catalogued +75 kcal a day it gives 78 g of tilapia,
   1.2 ug of B12, half the need (data/home_outline.json already noticed; the note never changed).
   Proposal: the note says "about half of one person's B12 at this yield".
5. **The greenhouse air is wetter than the carrot wants** (75 percent against plants.csv's 0.70).
   Either the carrot's range is too tight or the carrot belongs in a drier bed.
6. **The game has no micronutrient model at all** (crop_nutrition.ron's trap 3: "the game can say
   whether you grew enough calories and never whether you grew enough vitamin C"). The solo home's
   garden is sized by calories; this design shows the vitamins, minerals and fats add half again.
   The data now exists (data/food/crop_micronutrients.json); the code does not (section 10).
7. **NASA's 40 to 50 m2 a person is a calorie-and-air figure.** Held to every DRI, the hydroponic
   garden here needs about 90 m2: the potato gains 2.6 times under NASA's lights, but the soybean,
   which carries the zinc, choline and fats, only 1.5 times. Choline alone, an AI rather than an
   RDA, costs the comfort garden about 23 m2 (93 m2 with it, 70 without).

## 10. Proposals and requests

Data (not applied in this task; each is a small, separate change):

- plants.csv `bean` growth_days from 60 to 85 (finding 1).
- plants.csv `sweet_potato`: yield 3.297 items (0.989 kg) a plant and area_per_plant_m2 0.4645
  from NCSU CEFS (19,000 lb per acre, 8,712 plants per acre at 12 x 60 in), cited in yields.ron.
  It already sits in this design's candidates as a proposed row; it loses to the potato on area.
- The survival garden block in the homestead's arrays (section 7).
- Rename `potato_grow_bed` to `grow_bed` everywhere it appears: it grows any crop, and the names
  rule says rename the code with the word.
- grow_media.ron's aquaponic note (finding 4).
- A high-tunnel picking window for tomato, pepper and cucumber (finding 3).

Requests for game systems (src/, not edited here):

1. **Micronutrients in the food model.** Read data/food/crop_micronutrients.json and
   data/food/nutrient_requirements.json beside crop_nutrition.ron, so the Home page's food loop shows
   each nutrient's coverage, not only kcal, and names the gaps a garden cannot close.
2. **The UV flash.** A step after a button or oyster mushroom harvest (a UV lamp on the mushroom
   rack) that makes the harvest carry vitamin D, and a B12 supplement item in the ship's stores.
3. **The sunline as the drum's light**, on the solar dial's 12 h at 28 mol per m2 a day (the master
   plan's question 3), so the garden's yields here are the yields the game grows.
4. **Salt and iodized salt as consumables** in the ship's stores and the fleet ledger.

## 11. Scaling to N residents

Every target scales with people, so the linear program's areas scale exactly: N residents need N
times the area. What does not:

- **Whole plots.** Rounding up costs about 1 m2 a crop whoever lives there, so the area a resident
  falls slightly: 135.0 m2 for one, 134.5 for two, 134.4 for eight (generated block).
- **The vitamin D shelf.** One 0.72 m2 shelf gives 223 g of mushrooms a day, enough for about one
  and a half people: 1, 2, 3 and 6 shelves for 1, 2, 4 and 8.
- **The nursery, the seed stock and the tools** are per garden, not per person, and the oil press,
  grain mill and seed bank are per neighbourhood.
- **Labour batches.** One resident sows about twelve plots a week, eight residents about ninety,
  in the same weekly round, so the work grows more slowly than the beds.
- **Air and heat do not scale with the garden alone.** The garden's light and its latent heat scale
  with area, but the home's air volume and thermal mass do not; ship-life-support.md's per-home
  model is where that is settled, and the drum's 393 million m3 of air buffers it.
- **Resilience improves with N.** Four crops for one person is four single points of failure; a
  neighbourhood's gardens can trade a failed crop.

## 12. Not confirmed, refuted, and estimates

The numbers that size the garden were read twice (the last item below). What did not survive, what
was read only at second hand, and what was never sourced:

- **Refuted:** the often-quoted "20 to 40 mg of iodine per kg of salt" is WHO's 1996 figure for
  10 g of salt a day; WHO's 2014 guideline scales by intake (49 mg/kg at 4 g). Spirulina as a B12
  source (Watanabe et al. 1999). This design's own first run, sized to selenium, which the bed
  decides (section 1). The first runs' cucumber-led and carrot-led gardens, products of finding 3
  and of having no cap on food eaten.
- **Read through a second source, not the original:** WHO's 2014 salt table, read as reproduced in
  Greenwald et al. 2022 (WHO's own PDF refused automated reading). The journal articles were read
  as their abstracts through Europe PMC, not in full. The DRI and IOM pages were read through a
  summarising reader, twice with different questions, and agreed both times.
- **Not confirmed in this pass:** that biennial carrot and broccoli need a cold spell to set seed;
  that a long-day onion will not bulb at 12 h; UV-B dosing for skin; peanut storage moisture;
  whether a ship's medium carries enough iodine; whether the garden's residue is enough mushroom
  substrate; the salt build-up rate in a closed bed.
- **Estimates in the data, to replace with sourced figures:** most seed fractions and storage
  losses, the caps on food eaten, the potato's 5 percent refuse, the 97 percent UL margin, and the
  tuber density behind the potato's seed fraction.
- **Second reading of the numbers that size the garden:** USDA values for potato, soybean, broccoli
  and carrot were re-read through the FoodData Central API and match the CSV release to the digit;
  BVAD's rows were checked dry against fresh (each row's dry over one minus its water content gives
  its fresh figure, except sweet potato, finding 2); the CEFS rows by their own per-bed column (298
  lb per 100 ft bed x 87.12 = 26,000 lb per acre).

## 13. Questions for the operator

1. **Which garden does a new homesteader start with?** *Recommendation:* the bare four-crop garden in
   soil beds: it uses machines that exist, teaches the minimum, and the comfort garden is the upgrade
   when hydroponic machines arrive.
2. **Keep the caps on food eaten** (3 kg a day, 1 kg of any one crop)? *Recommendation:* yes, stated
   as estimates. Without them the smallest garden is a diet of about 2 kg of carrots a day.
3. **B12 in the game:** the ship's supplement, the fish tank, or both? *Recommendation:* a B12 tablet
   in the ship's stores as the default, the fish tank as the homesteader's option, never spirulina.
4. **Vitamin D:** a UV lamp over a mushroom shelf, a supplement, or both? *Recommendation:* both; the
   shelf is the self-reliant way and teaches something real.
5. **Whose targets?** The design holds the higher of the male and female DRI, so it feeds anyone;
   female-only targets would save 16 percent of the smallest bare garden and 19 percent of the
   comfort one (106 against 126 m2, 75 against 93). *Recommendation:* per-resident targets once the
   game knows a resident's sex and age; the higher values until then.
6. **The mushroom room in the minimal garden?** *Recommendation:* show it as the variant until the
   substrate is modelled, then adopt it: it cuts the lit area by about a quarter.
7. **The sunline: 12 h at 28 mol a day** as the drum's day (the master plan's question 3)?
   *Recommendation:* yes, it is NASA's staple-crop regime and the game's dial already.
8. **The plants.csv and layout proposals** (section 10): approve, and an agent applies them with the
   tests the crop files already carry.

## 14. Sources (read 2026-11-06 unless marked; the source's own date after its title)

- USDA FoodData Central, SR Legacy, April 2018 CSV release,
  https://fdc.nal.usda.gov/fdc-datasets/FoodData_Central_sr_legacy_food_csv_2018-04.zip; FDC ids
  per crop in data/food/crop_micronutrients.json; the four crops that size the bare garden re-read
  through https://api.nal.usda.gov/fdc/v1/foods.
- USDA National Nutrient Database for Standard Reference, Release 28 (2015), FOOD_DES.txt refuse,
  https://www.ars.usda.gov/ARSUserFiles/80400535/DATA/SR/sr28/dnload/sr28asc.zip.
- USDA Table of Nutrient Retention Factors, Release 6 (December 2007),
  https://www.ars.usda.gov/ARSUserFiles/80400525/Data/retn/retn06.txt.
- National Academies, Dietary Reference Intakes for Sodium and Potassium (2019), Appendix J, DRI
  Summary Tables, https://www.nationalacademies.org/read/25353/chapter/28.
- Institute of Medicine, Dietary Reference Intakes for Vitamin A, Vitamin K, ... Iron, Manganese,
  ... and Zinc (2001): chapter 9 Iron, https://www.nationalacademies.org/read/10026/chapter/11
  ("Hence the requirement for iron is 1.8 times higher for vegetarians."; "Based on a UL of 45
  mg/day of iron for adults, the risk of adverse effects from dietary sources appears to be low.");
  chapter 10 Manganese, https://www.nationalacademies.org/read/10026/chapter/12 ("a UL of 11 mg/day
  of total manganese intake from food, water, and supplements"); chapter 12 Zinc,
  https://www.nationalacademies.org/read/10026/chapter/14 ("The requirement for dietary zinc may be
  as much as 50 percent greater for vegetarians").
- FAO, Dietary protein quality evaluation in human nutrition, Food and Nutrition Paper 92 (2013),
  Table 3, https://www.fao.org/ag/humannutrition/35978-02317b979a686a57aa4593304ffc17f06.pdf.
- Anderson, Ewert and Keener (eds.), Life Support Baseline Values and Assumptions Document,
  NASA/TP-2015-218570/REV2 (February 2022), Tables 4-89, 4-90, 4-91,
  https://ntrs.nasa.gov/api/citations/20210024855/downloads/BVAD_2.15.22-final.pdf.
- Baldwin, NCSU CEFS Vegetable Planting Guide (2011),
  https://cefs.ncsu.edu/wp-content/uploads/sfu-vegetable-planting-guide.pdf.
- Schuh and Tong, UMN Extension, "Growing potatoes in home gardens" (reviewed 2026),
  https://extension.umn.edu/vegetables/growing-potatoes.
- Greenwald, Childs, Pachón, Timmer, Houston and Codling, "Comparison of Salt Iodization
  Requirements in National Standards with Global Guidelines", Curr Dev Nutr 6: nzac116 (2 August
  2022), Table 1 reproducing WHO 2014, read through Europe PMC (PMC9362757).
- Cardwell, Bornman, James and Black, "A Review of Mushrooms as a Potential Source of Dietary
  Vitamin D", Nutrients 10: 1498 (2018), abstract through Europe PMC.
- Martens, Barg, Warren and Jahn, "Microbial production of vitamin B12", Appl Microbiol Biotechnol
  58: 275-285 (2002), abstract through Europe PMC.
- Watanabe et al., "Pseudovitamin B12 is the predominant cobamide of an algal health food,
  spirulina tablets", J Agric Food Chem 47: 4736-4741 (1999), abstract through Europe PMC.
- Burdge and Calder, "Conversion of alpha-linolenic acid to longer-chain polyunsaturated fatty acids
  in human adults", Reprod Nutr Dev 45: 581-597 (2005), abstract through Europe PMC.
- Doughman, Krupanidhi and Sanjeevi, "Omega-3 fatty acids for nutrition and medicine: considering
  microalgae oil as a vegetarian source of EPA and DHA", Curr Diabetes Rev 3: 198-203 (2007),
  abstract through Europe PMC.
- Alfthan et al., "Effects of nationwide addition of selenium to fertilizers on foods, and animal and
  human health in Finland", J Trace Elem Med Biol 31: 142-147 (2015), abstract through Europe PMC.
- Kumar, Mahato, Kamle, Mohanta and Kang, "Aflatoxins: A Global Concern for Food Safety, Human Health
  and Their Management", Front Microbiol 7: 2170 (2016), abstract through Europe PMC.
- Royse, "Cultivation of Oyster Mushrooms", Penn State College of Agricultural Sciences (2003),
  https://www.mushroomcompany.com/resources/oyster/paoyster.pdf (the PDF read with pdftotext).
- Magnuson, EXTOXNET, "Protease inhibitors" (University of Idaho, summer 1997),
  https://extoxnet.orst.edu/faqs/natural/pro.htm.
- Blakeslee, Kansas State University, "Cooking dry beans safely" (13 October 2017),
  https://www.k-state.edu/news/you-asked-it/articles/2017/10/cooking-dry-beans-safely.html.
- Abbey, Penn State Extension, "Poison Hemlock" (updated 19 September 2025),
  https://extension.psu.edu/poison-hemlock.
- Repository data the design stands on: data/closed_loop/person.ron and sources.ron (bvad-2022,
  kusuma-2020, salisbury-1997, johnson-wheeler-2021), data/plants.csv, data/garden/yields.ron,
  harvest_windows.ron, lighting.ron, grow_media.ron, data/food/crop_nutrition.ron,
  data/machines/home_solo.ron, data/homes/homestead.ron; design documents
  [closed-loop-test.md](closed-loop-test.md), [ship-life-support.md](ship-life-support.md),
  [homestead-solo-design.md](homestead-solo-design.md), [homestead.md](homestead.md),
  [farm-clock.md](farm-clock.md), [two-realities.md](two-realities.md).
