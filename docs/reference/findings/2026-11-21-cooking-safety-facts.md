# Cooking safety facts for cooking rung 1

**Researched 2026-11-21.** The question: which safe cooking temperatures, times and nutrient
losses can the game teach in its cooking procedure (docs/design/cooking.md), from which primary
sources, and what did a separate adversarial refutation pass change?

## Short answer

- Meat, poultry, fish and leftovers: CDC's list (poultry 165 F, ground meat 160 F, whole cuts
  145 F then 3 minutes' rest, fish with fins 145 F or until opaque and separating with a fork,
  leftovers and casseroles 165 F). CDC gives Fahrenheit only; our Celsius is a conversion (74, 71,
  63, 63, 74 C).
- Eggs: CDC gives "firm" for eggs cooked whole and 160 F (71 C) for egg dishes. The game holds a
  whole egg's centre to that same 71 C, and says on the card that this is its reading.
- Dry beans: FDA says boil at least 30 minutes and that slow cookers at 75 C or less do not
  destroy the kidney-bean toxin. FDA gives no safe line between 75 C and the boil; the game counts
  beans kept below a full boil as unsafe, a cautious game choice, labelled as one.
- Raw dough, batter and raw egg: on CDC's riskier-foods list with no temperature; the game counts a
  dough dish safe once it is done.
- What overcooking costs: USDA's retention factors (Release 6, 2007) applied a second time, a game
  model labelled as one, except beans, which use USDA's own long-cooking row.
- The refutation pass confirmed every number and quotation it could reach and corrected wording in
  eleven places (below); nothing was refuted outright.

## Findings

**1. Safe internal temperatures.** CDC, "Preventing Food Poisoning",
https://www.cdc.gov/food-safety/prevention/index.html, dated Nov. 24, 2025, read 2026-11-21.
"All poultry, including ground chicken and turkey": "165°F"; "Ground meats, such as beef and
pork": "160°F"; "145°F (then allow the meat to rest for 3 minutes before carving or eating)";
"Fish with fins": "145°F or cook until the flesh is opaque and separates easily with a fork";
"Leftovers and casseroles": "165°F". Refutation: CONFIRMED. 160 F is 71.1 C, so 71 C is 0.1 C
under; kept because CDC itself pairs "160°F (71°C)" for egg dishes, and the game has no
ground-meat dish yet.

**2. The thermometer, and its exception.** Same page: "The only way to tell if food is safely
cooked is to use a food thermometer." Its next sentence: "You can't tell if food is safely cooked
by checking its color and texture (except for seafood)." Refutation: CORRECTED: the game's words
had dropped the seafood exception and applied "colour does not tell you" to eggs; the card, the
how-to and the quest now say "for meat and poultry".

**3. Eggs.** CDC, "Backyard Poultry",
https://www.cdc.gov/healthy-pets/about/backyard-poultry.html, dated July 18, 2025, read
2026-11-21: "Cook eggs until both the yolk and white are firm." and "Egg dishes should be cooked to
an internal temperature of 160°F (71°C) or hotter." Refutation: quotes CONFIRMED; the 71 C reading
for a whole egg is defensible but was labelled only in a data comment. CORRECTED: the card now
says "safe once the yolk and white are firm; the game counts that as 71 C (160 F) at the centre,
the CDC's line for egg dishes" (`cooking.safe_eggs`).

**4. Dry beans.** FDA, "Bad Bug Book", 2nd ed. (2012), chapter "Phytohaemagglutinin (kidney bean
lectin)", https://www.fda.gov/media/83271/download, read 2026-11-21 (PDF, text extracted
locally): "Consumers should boil the beans for at least 30 minutes"; "boiling the beans for 10
minutes (100°C) completely destroyed the toxin" (Bender and Readi); "internal temperatures of only
75°C or less, which is inadequate"; its consumer box: "soaked in water for at least 5 hours, the
water poured away, and the beans boiled in fresh water for at least 30 minutes". Refutation:
quotes CONFIRMED. CORRECTED: "below the boil they stay unsafe" was the game's, not FDA's (the
game's low heat is a 90 C simmer, which FDA does not address), and the advice is for kidney beans;
the card, recipe and data now say both.

**5. Raw dough and raw egg.** CDC, "Preventing Food Poisoning" (finding 1), its riskier foods:
"Raw dough or raw batter made with uncooked flour" and "Raw or undercooked poultry, meat, seafood,
or eggs". Refutation: raised by the pass (the first draft called undercooked dough "safe, but not
cooked through", less safe than CDC). CORRECTED: the dough class is safe only once done, and a
dish with raw egg in it keeps the eggs' risk undercooked.

**6. Nutrient retention.** USDA ARS Nutrient Data Laboratory, "USDA Table of Nutrient Retention
Factors, Release 6", December 2007,
https://www.ars.usda.gov/ARSUserFiles/80400525/Data/retn/retn06.pdf, read 2026-11-21 (PDF, text
extracted locally with pdftotext -raw; the -layout extraction misaligns rows). It defines "True
retention" as "the proportion of the nutrient remaining in the cooked food in relation to the
nutrient originally present in the raw food". Rows used (vitamin C; mean of thiamin, riboflavin,
niacin, B-6, folate total, B-12): 0805 CHICKEN,ROASTED 0.80 / 0.74; 2305 FINFISH,<5%FAT,BROILED,
WO/DRIPPINGS 0.80 / 0.92; 0105 EGGS,HARD COOKED 0.80 / 0.88 (0103 EGGS,FRIED,SCRAMBLED the same);
3776 VEG,OTHER,BLD,WATER USED 0.85 / 0.93; 0358 CEREAL,REG/QUICK,COOKED 0.80 / 0.87 (porridge);
0432 RICE,WHITE/BROWN,COOKED,WATER USED 0.80 / 0.89; 0381 PASTA,BOILED,DRAINED 0.70 / 0.76; beans
0541 LEGUMES,CKD 2/2.5HRS over 0521 LEGUMES,CKD 45/75MIN: 1.00 / 0.86. Refutation: CONFIRMED,
alignment proved independently (the alcohol column of rows 5001 to 5010 reads the published
series); CORRECTED: rice and pasta had borrowed the cereal row, and beans' second-cook model
contradicted the table's constant legume vitamin C, so beans now use the sourced long-cooking ratio.

**7. Cooked kidney beans.** USDA FoodData Central, SR Legacy NDB 16028 "Beans, kidney, all types,
mature seeds, cooked, boiled, without salt" (FDC 173740, published 2019-04-01), read 2026-11-21
through the FDC search API: "Energy 127 KCAL", "Protein 8.67 G", "Iron, Fe 2.22 MG". Refutation:
CONFIRMED (2.94 mg iron belongs to NDB 16328, the salted row).

**8. When to see a doctor.** CDC, "Symptoms of Food Poisoning",
https://www.cdc.gov/food-safety/signs-symptoms/index.html, dated Nov. 24, 2025: "See a doctor if
you have any symptoms that are severe, including:" bloody diarrhea, diarrhea more than 3 days,
"High fever (temperature over 102°F)", vomiting so often liquids stay down, signs of dehydration.
Refutation: CONFIRMED (38.9 C is our conversion).

## Game choices, labelled as such in the data

The temperatures around the food (a pan at 160 C, an oven at 180 C; boiling at 100 C is physics),
how fast each medium heats food, each class's heat-up time, tenderising time, overcook and burn
times, the second-cook vitamin model, the spoilage hours of the two new cooked profiles, and the
risk odds (`raw_consumption_risk`, already labelled game-scale). None is taught as a sourced figure.

## Still unknown, and what would settle it

- FSIS's own chart (yolk set temperature, resting, placement of the thermometer): fsis.usda.gov
  refuses automated reading; read by hand. It would let the game give a whole egg its own line.
- FoodKeeper's hard-cooked egg and cooked bean storage times: the FoodKeeper data, read by hand or
  from FSIS's download; until then the two profiles keep 72 h as a game choice.
- A safe line for beans between 75 C and the boil: a primary study of lectin destruction by
  temperature and time (FDA cites "some variation in toxin stability"); none was read.

## Sources not reached

- fsis.usda.gov and foodsafety.gov: refuse automated reading (memory reference_research_sources);
  not tried this pass.
- https://www.cdc.gov/food-safety/foods/eggs.html and .../prevention/eggs.html: 404 on 2026-11-21;
  the egg lines came from CDC's Backyard Poultry page instead.

## What it means for the project

**Certain:** the safe temperatures, the egg lines, FDA's bean advice, the retention factors and
the cooked bean nutrition above are what their sources say, as of the dates given.

**Judgement calls:** counting a whole egg's centre at 71 C as "firm"; counting beans below a full
boil as unsafe at any time; applying kidney-bean advice to every dry bean; counting dough safe once
done; the second-cook vitamin model; every number in "Game choices".
