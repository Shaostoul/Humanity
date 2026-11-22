# Preserving safety facts for preserving rung 1

**Researched 2026-11-21.** The question: which drying, canning and salting figures can the game
teach in its preserving procedures (docs/design/preserving.md), from which primary sources, and
what did a separate adversarial refutation pass change? Canning teaches a procedure where a wrong
figure can kill (botulism in low-acid food), so every figure here was read from its primary page
and then re-read by a second reader trying to refute it.

**How the pages were read.** robots.txt first, in a step of its own, for every site (NCHFP, CDC,
FDA, Colorado State University Extension, University of Alaska Fairbanks Extension, University of
Kentucky, Wisconsin Sea Grant, USDA FoodData Central): none barred these pages. Requests identified
honestly. CDC refuses `curl` (HTTP 403) and was read through a fetching tool whose model quotes the
page; three separate reads agree word for word, but the CDC wording below should still be
spot-checked in a browser. NCHFP's pages carry no page date of their own; each names the USDA
source it was adapted from, quoted per figure.

## Short answer

- **Canning**: the USDA Complete Guide to Home Canning (2015 revision), as NCHFP prints it, gives
  the tested processes the game uses: applesauce in a boiling-water canner (pints 15 minutes at 0 to
  1,000 ft, longer higher), carrots (pints 25 minutes) and white potatoes (pints 35 minutes) in a
  pressure canner at 10 lb weighted gauge (0 to 1,000 ft, 15 lb above) or 11 to 14 lb dial gauge by
  altitude, vented 10 minutes first. Low-acid food (pH above 4.6: every fresh vegetable but most
  tomatoes, meat, fish, milk) only in a pressure canner (CDC). Every figure CONFIRMED.
- **Botulism**: CDC's symptoms, "a medical emergency", antitoxin, weeks to months in hospital, fewer
  than 5 in 100 die, the toxin "cannot be seen, smelled, or tasted". No current CDC botulism page
  gives an onset time; FDA's Bad Bug Book (2012) does (18 to 36 hours, 4 hours to 8 days, adults).
- **Drying**: 60 C (140 F) in an oven or dehydrator; the sun at 85 F or warmer and humidity under 60%,
  fruit only; home-dried fruit about 20% water, vegetables about 10%; apples 6 to 24 hours, carrots
  6 to 10; dried fruit keeps a year at 60 F, six months at 80 F, vegetables about half. CONFIRMED,
  with four dropped conditions put back (below).
- **Salting**: the only complete, sourced home salt-by-weight procedure found is a dry-cured country
  ham (University of Kentucky). No primary source gives a safe dry-salt ratio for shelf-stable home
  salt fish. Salting is not taught in rung 1 (the ham's cure is about 60 days below 40 F; it needs
  the long-process clock, rung 2).
- **The refutation pass** confirmed 18 of 22 figures and corrected 4 (each had dropped a condition);
  nothing was refuted outright. Game choices are listed at the end.

## Findings

### Canning

**C1. The pH line.** NCHFP, "Ensuring Safe Canned Foods",
https://nchfp.uga.edu/how/can/general-information/ensuring-safe-canned-foods/ (no page date;
"Adapted from the "Complete Guide to Home Canning," Agriculture Information Bulletin No. 539,
NIFA-USDA (Revised 2015)."): "Acid foods have a pH of 4.6 or lower. They include fruits, pickles,
sauerkraut, jams, jellies, marmalades, and fruit butters."; "Low-acid foods have pH values higher
than 4.6. They include red meats, seafood, poultry, milk, and all fresh vegetables except for most
tomatoes."; figs and some tomatoes "have pH values slightly above 4.6" and "must be acidified".
Refutation: CORRECTED: say MOST fruits, not all. The USDA guide's Guide 2 has Asian pears acidified
and no tested process for melons, bananas, papaya, ripe mango or coconut as purees; CDC calls figs
low-acid outright. The game's words say "most fruit"; apples (the game's only canned fruit) have a
tested boiling-water process.

**C2. Pressure canner only, for low-acid food.** CDC, "Home-Canned Foods",
https://www.cdc.gov/botulism/prevention/home-canned-foods.html (Apr. 25, 2024): "Pressure canning
is the only recommended method for canning low-acid foods."; "Do not use a boiling water canner for
low-acid foods because it will not protect against botulism."; "Home-canned vegetables, which are
low-acid foods, are the most common cause of botulism outbreaks in the United States."
Refutation: CONFIRMED.

**C3. Why: the canner's temperature.** NCHFP, Ensuring Safe Canned Foods: "Botulinum spores are very
hard to destroy at boiling-water temperatures; the higher the canner temperature, the more easily
they are destroyed. Therefore, all low-acid foods should be sterilized at temperatures of 240° to
250°F, attainable with pressure canners operated at 10 to 15 PSIG." Refutation: CONFIRMED, with a
dropped condition put back: 10 to 15 PSIG gives 240 to 250 F at sea level; higher up needs more
pressure (the altitude bands below). NCHFP never says in so many words that 212 F fails; it says
low-acid food would need "7 to 11 hours" in boiling water, and CDC states the rule (C2).

**C4. Applesauce.** NCHFP, "Applesauce",
https://nchfp.uga.edu/how/can/canning-fruits-and-fruit-products/applesauce/ (no page date; adapted
from the USDA guide, revised 2015): hot pack, "leaving ½-inch headspace"; "Table 1. Recommended
process time for Applesauce in a boiling-water canner.": pints 15, 20, 20, 25 minutes and quarts
20, 25, 30, 35 at 0-1,000, 1,001-3,000, 3,001-6,000 and above 6,000 ft; weighted-gauge pressure
canner pints 8 and quarts 10 minutes at 5 lb (0-1,000 ft) or 10 lb (above); dial gauge 6, 7, 8,
9 lb by the four bands to 8,000 ft. "an average of 13½ pounds is needed per canner load of 9
pints". Refutation: CONFIRMED (and against Guide 2 of the 2015 guide).

**C5. Carrots.** NCHFP, "Carrots - Sliced or Diced",
https://nchfp.uga.edu/how/can/canning-vegetables-and-vegetable-products/carrots-sliced-or-diced/
(no page date; USDA guide, revised 2015): hot or raw pack, 1-inch headspace; pints 25 minutes,
quarts 30; weighted gauge 10 lb (0-1,000 ft), 15 lb (above); dial gauge 11, 12, 13, 14 lb at
0-2,000, 2,001-4,000, 4,001-6,000, 6,001-8,000 ft. "an average of 11 pounds is needed per canner
load of 9 pints". Refutation: CONFIRMED.

**C6. White potatoes.** NCHFP, "Potatoes, White - Cubed or Whole",
https://nchfp.uga.edu/how/can/canning-vegetables-and-vegetable-products/potatoes-white-cubed-or-whole/
(no page date; USDA guide, revised 2015): hot pack only, 1-inch headspace, pints 35 minutes, quarts
40, pressures as carrots; "Do not use the water you cooked the potatoes in; it contains too much
starch." "an average of 13 pounds is needed per canner load of 9 pints". Refutation: CORRECTED: the
times hold for the tested preparation only, "1/2-inch cubes" boiled 2 minutes, or whole potatoes
"1 to 2 inches in diameter" boiled 10 minutes. The recipe's words say half-inch cubes boiled 2
minutes.

**C7. Using the canners.** USDA guide, Guide 1 (2015),
https://nchfp.uga.edu/papers/guide/GUIDE01_HomeCan_rev0715.pdf, and NCHFP, "How to Use Boiling
Water and Pressure Canners" (whose own note says "revised 2009", so each step was checked against
Guide 1): "To be safe, all types of pressure canners must be vented 10 minutes before they are
pressurized."; "Start timing the process when the pressure reading on the dial gauge indicates
that the recommended pressure has been reached, or when the weighted gauge begins to jiggle or
rock"; "If at any time pressure goes below the recommended amount, bring the canner back to
pressure and begin the timing of the process over, from the beginning"; "Do not force-cool the
canner." Boiling water: "at least 1 inch above jar tops. For process times over 30 minutes, the
water level should be at least 2 inches above the tops of the jars."; "If the water stops boiling
at any time during the process, bring the water back to a vigorous boil and begin the timing of
the process over, from the beginning." Guide 1's glossary: venting "is necessary to prevent a risk
of botulism in low-acid canned foods". Refutation: CONFIRMED.

**C8. Half-pints.** NCHFP, "Selecting the Correct Processing Time" (no page date; USDA guide 2015):
"Process times for ½-pint and pint jars are the same, as are times for 1-½ pint and quart jars."
Refutation: CONFIRMED.

**C9. Storing.** NCHFP, "Storing Home Canned Foods" (no page date; USDA guide 2015 and So Easy to
Preserve, 6th ed.): "For best quality, store between 50 and 70 °F."; "can no more food than you
will use within a year"; "Do not store jars above 95° F". Ensuring Safe Canned Foods: "Properly
sterilized canned food will be free of spoilage if lids seal and jars are stored below 95°F."
Unsealed jars: Guide 1 and NCHFP's "Cooling Jars and Testing Jar Seals": "Foods in single unsealed
jars could be stored in the refrigerator and consumed within several days." (the storing page says
"within one week"; the shorter is kept). Refutation: CORRECTED: 50 to 70 F and "within a year" are
QUALITY figures, not safety ones; reprocessing within 24 hours is safe only if the first canning
followed a tested recipe, with a clean jar, a new lid and a tested hot-pack process, and NCHFP calls
it "not recommended" for quality. The game's year on a jar is labelled a quality limit; unsealed jars
are rung 3.

**C10. Salt.** Guide 1 (2015), p. 1-31: "salt seasons the food but is not necessary to ensure its
safety." Refutation: CONFIRMED. The game's canning recipes carry no salt.

### Botulism

**B1.** CDC, "Symptoms of Botulism", https://www.cdc.gov/botulism/signs-symptoms/index.html (April 1,
2024): "Difficulty swallowing", "Muscle weakness", "Double vision", "Drooping eyelids", "Blurry
vision", "Slurred speech", "Difficulty breathing", "Difficulty moving the eyes"; foodborne may add
"Vomiting", "Nausea", "Stomach pain", "Diarrhea"; "Botulism is a medical emergency." CDC,
"Treatment of Botulism", https://www.cdc.gov/botulism/treatment/index.html (April 17, 2024):
"Botulism is treated with antitoxin, which prevents the toxin from causing any more harm.";
"Antitoxin works best when given early in the illness."; "Some people need to stay in the hospital
for weeks or even months before they are well enough to go home."; "Today, fewer than 5 of every
100 people with botulism die." CDC, Home-Canned Foods: "You cannot see, smell, or taste the toxin
that causes botulism."; "If in doubt, throw it out! Never taste food to see if it's safe."; the
signs of a bad container. Refutation: CONFIRMED (CDC through the fetching tool, see above).

**B2. Onset.** FDA, "Bad Bug Book", 2nd ed. (2012),
https://www.fda.gov/files/food/published/Bad-Bug-Book-2nd-Edition-(PDF).pdf: "Onset: Adult: Usually
18 to 36 hours after ingesting food containing the toxin, although times have varied from 4 hours
to 8 days." Refutation: CONFIRMED, adults only; no current CDC botulism page (seven pages and the
2021 clinical guidelines checked) gives an onset time; a 2021 CDC museum newsletter says "18 to 36
hours". Rung 1 starts the illness when the jar is eaten and says so; the wait is rung 2.

### Drying

**D1. Temperature.** Colorado State University Extension, "Drying Fruits",
https://extension.colostate.edu/resource/drying-fruits/ (published August 1994, reviewed August
2025): "Dry at 140 degrees F (60°C) in an oven or dehydrator." NCHFP, "Food Dehydrators" (So Easy to
Preserve, 6th ed. 2014): "Dehydrators are efficiently designed to dry foods fast at 140ºF."
Refutation: CONFIRMED.

**D2. The sun.** NCHFP, "Sun Drying", https://nchfp.uga.edu/how/dry/drying-general/sun-drying/ (So
Easy to Preserve, 6th ed. 2014): "A minimum temperature of 85ºF is needed with higher temperatures
being better."; "A humidity below 60 percent is best."; "It takes several days to dry foods
out-of-doors."; "The high sugar and acid content of fruits make them safe to dry out-of-doors";
"Vegetables (with the exception of vine dried beans) and meats are not recommended for
out-of-doors drying."; "Fruits dried out-of-doors must be covered or brought under shelter at
night." Refutation: CORRECTED: sun-dried fruit needs treating against insects and their eggs, in a
freezer at 0 F or below for at least 48 hours or an oven at 160 F for 30 minutes (NCHFP's Vine
Drying page; UAF FNH-00745 says the same). The card says it.

**D3. Apples.** Colorado State, Drying Fruits (NCHFP's drying page links to it; NCHFP has no apple
page): "Cut in rings or slices 1/8 to 1/4 inch thick"; "Dip in ascorbic acid or other
antidarkening/antimicrobial solution for 10 minutes."; "Pretreating fruits prior to drying is highly
recommended."; "Dry until soft, pliable, and leathery; no moist area in center when cut (6-24
hours)." Refutation: CONFIRMED.

**D4. Carrots.** Colorado State, "Drying Vegetables",
https://extension.colostate.edu/resource/drying-vegetables/ (published October 1998, reviewed
November 2012), Table 2: "Cut in slices or strips 1/8″ thick."; blanch "4" minutes; dry "6-10"
hours; "Tough to brittle". Refutation: CONFIRMED, with the table's note in full: blanching times are
for 3,000 to 5,000 ft, "slightly shorter for lower altitudes" and slightly longer for higher
altitudes or for large quantities.

**D5. How dry.** NCHFP, "Packaging and Storing Dried Foods",
https://nchfp.uga.edu/how/dry/drying-general/packaging-and-storing-dried-foods/ (So Easy to
Preserve, 6th ed. 2014): "The moisture content of home dried fruit should be about 20 percent.";
vegetables "should contain about 10 percent moisture". Refutation: CONFIRMED.

**D6. Conditioning.** The same page: "Seal the containers and let them stand for 7 to 10 days.";
"Conditioning is the process used to equalize the moisture. It reduces the risk of mold growth."
Refutation: CONFIRMED, with the last step put back: if condensation forms, dry the fruit more.
Conditioning is rung 2.

**D7. How long it keeps.** The same page: "Most dried fruits can be stored for 1 year at 60ºF, 6
months at 80ºF."; "Vegetables have about half the shelf-life of fruits." (Colorado State says 6 to
12 months for both; the shorter is kept.) Refutation: CONFIRMED.

**D8. Water, energy and vitamin C.** USDA FoodData Central, SR Legacy (publication date 4/1/2019),
read through its API 2026-11-21: apples raw with skin (NDB 09003) water 85.56 g; carrots raw (11124)
88.29 g; applesauce canned unsweetened without added ascorbic acid (09019) 88.22 g; carrots canned
regular pack drained solids (11128) 92.95 g; potatoes canned drained solids (11376) 84.28 g;
apples dehydrated low moisture sulfured uncooked (09009) 3 g; carrot dehydrated (11683) 4 g.
Refutation: CONFIRMED to one decimal from FDC's own site data (its API's shared demo key was spent),
energy and vitamin C matching for all six checked.

### Salting (recorded for rung 2, not taught in rung 1)

**S1. Country ham.** University of Kentucky Cooperative Extension, ASC-213, "How to Make a Country
Ham", https://publications.mgcafe.uky.edu/files/ASC213.pdf (issued 3-2014): "Cures 100 pounds of
ham." with "8 pounds salt" and "2 pounds of sugar"; "at two days per pound of ham or another rule
of thumb is 60 days"; "During the curing process, the ham must stay cold (<40°F)."; "A ham must
have lost at least 18 percent of its green weight and contain at least 4 percent salt".

**S2. What is not found.** FDA's Fish and Fishery Products Hazards and Controls Guidance, chapter 13,
https://www.fda.gov/media/80310/download, gives processor end points, not a home recipe ("20%
water phase salt" for shelf-stable salted fish). University of Alaska Fairbanks, FNH-00325
(revised February 2021): "Today's lightly salted and smoked fish is not a preserved product".
No primary source read gives a safe dry-salt ratio for shelf-stable home salt fish. Montana State
and Oregon State Extension bar automated readers in robots.txt; UC ANR (403), Oklahoma State (a bot
check) and NOAA's Fishery Leaflet 18 (no PDF served) could not be read.

## Game choices (labelled so in the data)

- The drying rates per procedure (apples 0.32, carrots 0.5 per hour on the thin-layer curve), sized
  so the time to dry falls inside the source's range; the sun's share (0.15, about 66 hours for
  apples, "several days"). Humidity is not modelled.
- The home's elevation, 0 ft (the home's air is at sea-level pressure).
- The chance of Botulism from one jar with the risk (0.5): no source gives a chance per jar. Kept
  as built; the operator has not weighed this number.
- Botulism's course (7 game days) and weakness (speed halved); it starts when the jar is eaten.
  Kept as built.
- Botulism's severity by the Illness and injury mode (the operator's call, 2026-11-21, "Yes" to the
  orchestrator's recommendation: fatal in Realistic unless treated at a medbay, serious but
  survivable in Simplified, the setting's Forgiving). The numbers that carry it are the game's:
  1 health an hour in Realistic (a full body's 100 health gone in about 4 of the 7 game days), and
  in Forgiving half that over half the course, never below a quarter of full health
  (data/medical/illnesses.ron). The medbay's treatment, reached in a medbay (General Services' medbay;
  the Medical Bay district has no rooms yet), ends it at once, standing for the antitoxin and hospital care CDC
  describes. No source gives a death rate for untreated botulism in the terms a game needs; CDC's
  "fewer than 5 of every 100 people with botulism die" is with treatment. (The harm before this
  call was 0.4 health an hour in every mode, from status_effects.csv.)
- The drained weight of a pint of carrots or potatoes (300 g) and of applesauce (440 g).
- An under-dried food or an under-processed acid jar keeping only as long as its fresh food.
