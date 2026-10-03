# Food land per person, and how fast goods and people move: findings

**Research date: 2026-10-03.** Every source below was read on 2026-10-03. This
is a reading of public sources, done to give the game honest numbers. **It is
not agronomic, engineering or planning advice**, and nobody who wrote it is an
agronomist or a transport engineer. Before sizing a real garden or a real
freight system, check the primary sources and a local professional.

**The questions.** Two questions, asked together by the appendix of
[`ship-homes-and-logistics.md`](../../design/ship-homes-and-logistics.md),
which found the project's own figures unsourced:

1. How much growing area does one person need for a complete year's diet,
   under (a) field biointensive growing, (b) conventional agriculture, and
   (c) controlled-environment growing (vertical farms, NASA and Chinese and
   European bioregenerative life support)?
2. How fast do goods and people really move inside a large facility or city
   and between sites (walking, freight lifts, conveyors, pneumatic tubes,
   robots, light rail and metro, maglev), and how long does loading and
   handling take? And how do the speeds in
   [`data/transportation.ron`](../../../data/transportation.ron) compare?

Prepared for:

- [`self-sufficiency.md`](../../design/self-sufficiency.md) line 82, which says
  "~700-1000 m2 of intensive growing feeds **one** person a complete diet for a
  year" with no source;
- [`homestead-solo-design.md`](../../design/homestead-solo-design.md) lines
  167 to 175, which uses that figure to size the 1,156 m2 garden room as a
  one-person food engine (the "canopy cap");
- `data/home_outline.json` line 107, which compares the home greenhouse against
  "Biointensive at 700 to 1,000 m2 per person";
- `data/transportation.ron`, whose speeds carry no sources.

---

## Short answer

**Food land.**

- **No source was found for 700 to 1,000 m2.** It is about two to four times
  each figure used from Ecology Action, the organisation behind biointensive
  growing (232 to 372 m2; F2 and F3 state intermediate yields, F1 states
  none). Its figures for beginning yields were not read.
- **Biointensive, sun-lit, soil beds: about 230 to 370 m2 per person**, for a
  complete vegan diet including the compost crops that keep the soil fertile.
  Ecology Action's book figure is 4,000 sq ft (372 m2) "with soil fertility
  sustained" at intermediate yields; its staff's 2019 answer is 2,500 sq ft
  (232 m2) including paths, for a person eating 2,400 calories a day; its
  older claims are 2,800 and 3,403 sq ft (260 and 316 m2).
- **The one measured closed trial agrees.** Biosphere 2's 2,000 m2 of crops
  gave eight people "about 80 percent of overall nutritional needs" for two
  years: 250 m2 per person for 80 percent, while the crew lost 10 to 20
  percent of their body weight.
- **Conventional agriculture uses several times more.** Ten US diet scenarios
  (two based on actual consumption, eight that meet dietary guidelines) need
  0.13 to 1.08 ha (1,300 to 10,800 m2) per person per year (Peters et al.
  2016). The world has 0.17 ha (1,717 m2) of arable land per person
  (World Bank, from FAO data, 2023), and all agricultural land including
  grazing comes to about 5,900 m2 per person (computed from the same data).
- **Under strong lamps with added CO2, the area falls to tens of square
  metres, and the cost moves to electricity.** Wheat alone: 12 to 30 m2 per
  person (Salisbury, Bugbee and Bubenheim 1987). NASA's planning figure for a
  self-sustaining colony is about 50 m2 per person (Wheeler). NASA's Baseline
  Values and Assumptions Document (2022) needs 65.29 m2 per crew member for
  its most-closed diet, which still ships in herbs, condiments and some
  nutrients. Lunar Palace 1 grew 73 percent of four people's food by dry
  weight (all of their plant food) on 120 m2 of stacked shelves, 30 m2 each.
- **Recommendation (a judgement call, see the end):** the game should drop 700
  to 1,000 and use two numbers keyed to the light source: about **370 m2 per
  person for sun-lit biointensive beds**, and about **50 to 65 m2 per person
  for lamp-lit chambers with CO2**, the second one paid for in electricity. On
  the sourced figure the 1,156 m2 garden room is roughly a **three-person**
  sun-lit food engine, not a one-person one.

**Speeds and handling.**

- **Most rows of `data/transportation.ron` sit inside real ranges.** Light
  rail, cargo rail, heavy rail (US Class 6 track allows 110 mph, 177 km/h,
  above the row's 160), maglev (500 km/h, and a 10 mm gap like the Transrapid
  system Shanghai uses), the conveyor, the AGV and the pneumatic tube are all
  within what real systems do. The dirt path's 2.0 m/s is above a normal
  walking pace (1.10 to 1.65 m/s), but it is a speed limit on a path that
  carries carts of up to 1 tonne, not a walking pace, so it is not counted
  as an outlier.
- **The design document's claim that the AGV and tube rows exceed real ones
  is wrong for both.** The tube row (8 m/s, 10 kg capsules, 200 mm tube) is
  faster and bigger than a typical six-inch hospital tube (5.5 to 7.6 m/s,
  5 lb), but a maker's large-bore hospital system carries "nearly 28 kg ... at
  speeds of 8 meters per second" in tubes up to 315 mm. The AGV row's 2.0 m/s
  carrying 1 tonne is inside a current product: Rockwell's OTTO 1500 is rated
  for 1,900 kg and a maximum speed of 2.0 m/s. Other heavy robots read are
  slower (1.2 to 1.3 m/s).
- **Real outliers:** the space elevator climbs at 200 m/s in the data against
  Edwards's "up to 200 km/hr" (55.6 m/s), which looks like a unit slip; the
  slurry pipeline's 3.0 m/s is nearly double the one real line read (its
  design velocity, 1.68 m/s).
- **The cargo lift depends on which lift it is.** The row is described as a
  lift "for multi-level facilities and mine shafts" with 100 m of travel. Its
  2.0 m/s is twice the top speed in a manufacturer's freight lift planning
  guide (1.0 m/s), but half a mining handbook's rule of thumb for the
  economic speed of a 100 m mine hoist (about 4 m/s).
- **Metros average 30 to 45 km/h including stops**, far below their top
  speeds, and a train at a busy station stands for 30 to 50 seconds. The
  trucking industry commonly takes **2 hours** as the average time to load
  or unload a truck; that is a working definition quoted by a study, which
  also says "there is currently no standard definition", not a measured
  average. A hospital tube crosses its longest route, 1,500 ft, in under
  three minutes. A sourced handling time for one freight transfer inside a
  ship is still missing.

---

## How the numbers were handled

- Square feet are converted at 1 ft2 = 0.09290304 m2, hectares at 1 ha =
  10,000 m2, feet per minute (fpm) at 1 fpm = 0.00508 m/s, miles per hour at 1
  mph = 0.44704 m/s. Every conversion is ours and is marked "(computed)".
- Quotations were copied from the source text itself, not from a summary:
  pages fetched through a fetch tool or downloaded with curl, and PDFs turned
  into text with pdftotext. An independent review the same day downloaded
  nearly every source again with curl and found the quotations word for word;
  what it corrected is listed at the end.
- **Quotations reproduce the source's words and numbers exactly.** Where PDF
  text extraction garbled a range dash it is written as an en dash; where it
  dropped a micro sign (µ) the sign is restored in square brackets.
- "Per person" means a whole year of food for one adult unless the source says
  otherwise. Several sources do say otherwise (part of a diet, wheat only,
  oxygen only), and each finding says which.

---

## Part 1: growing area per person

### A. Biointensive growing (Ecology Action / GROW BIOINTENSIVE)

**F1. Ecology Action's own history page.**
[growbiointensive.org/About_highlights.html](https://growbiointensive.org/About_highlights.html),
no page date (it lists highlights from 1972 to 2010). Two entries carry the
figures our design documents quote:

- 1978: "a complete vegetarian diet for one person being grown on as little as
  2,800 square feet." That is 260 m2 (computed).
- 1993: "Biosphere II, using techniques based on Ecology Action's work, raises
  80% of its food needs for the last 2 years within a 'closed system.' This
  experience demonstrates that a complete year's diet for one person could be
  raised on 3,403 square feet (1/6-1/13 of what commercial agriculture is using
  to feed one person)." That is 316 m2 (computed).
- Note what "as little as" means: these are best cases, not planning
  averages. The 3,403 figure is an extrapolation from Biosphere 2 (F4), not a
  separate trial.

**F2. The book figure: 4,000 sq ft at intermediate yields, fertility
sustained.** John Jeavons, *How to Grow More Vegetables*, Eighth Edition, Ten
Speed Press, 2012 (ISBN 9781607741909), page 214, Appendix 2, "The Efficacy of
the GROW BIOINTENSIVE Method", read from the publisher's excerpt
[content.randomhouse.com/.../Page214.pdf](https://content.randomhouse.com/assets/9781607741909/pdfs/Page214.pdf):

- "GROW BIOINTENSIVE intermediate yields with soil fertility sustained 4,000 sq
  ft". That is 372 m2 (computed). The heading says the diet includes "Crops
  That Produce a High Level of Calories per Unit of AREA".
- The same page's conventional figures, for comparison (all with fossil fuels
  available unless stated):
  - "High animal product diet (fossil fuels available) currently
    31,000–63,000 sq ft" (2,880 to 5,853 m2, computed);
  - "Average U.S. diets1 (fossil fuels available) currently 15,000–30,000 sq
    ft" (1,394 to 2,787 m2, computed);
  - "Average U.S. vegan (fossil fuels available) currently 7,000 sq ft" (650
    m2, computed);
  - "Average U.S. vegan diet (no animal products) (post-fossil fuel era)
    21,000–28,000 sq ft" (1,951 to 2,601 m2, computed).
- This is Ecology Action's planning figure, and it is the one that includes
  growing the compost crops needed to keep the beds fertile. **It is the most
  defensible single number for a sun-lit soil garden.**

**F3. Ecology Action's 2019 answer: 2,500 sq ft including paths, at 2,400
calories a day.** "FAQ from Facebook: Biointensive Calories per Acre", "By
Ecology Action Staff", Ecology Action e-newsletter, Summer 2019,
[growbiointensive.org/Enewsletter/Summer2019/caloriesFAQ.html](https://growbiointensive.org/Enewsletter/Summer2019/caloriesFAQ.html)
(the site refuses plain scripted requests with HTTP 403; it answered one
sent with ordinary browser headers):

- Who said what: John Jeavons signs only the short answer, "Probably ~15+ to
  30+ x 876,000 calories/acre with closed-loop GROW BIOINTENSIVE Sustainable
  Small-Scale Farming, growing nutritionally complete diets." The figures
  below are from the "Perspective" that follows it, written by Ecology Action
  staff.
- The calorie level: "One average person consumes 876,000 calories annually
  (2,400 calories per day x 365)."
- A "carefully designed vegan diet, as well as the compost materials to
  maintain and build soil fertility, can be grown in twenty 100-sq-ft
  Biointensive beds, assuming intermediate GB yields".
- "With space for one-foot-wide paths added, the total area needed per person
  comes to 2,500 sq ft". That is 232 m2 (computed).
- The same page says "one acre can allow the growing of 30 complete diets",
  which is 1,452 sq ft (135 m2) each (computed). That figure rests on a design
  not yet finished: "The Jeavons Center and Victory Gardens for Peace are
  developing a design that could grow all the food for a complete balanced
  diet for one person annually, as well as all the compost materials from the
  carefully chosen diet, in 10 beds." So it is reported here and not used.

**F4. Biosphere 2, the one measured closed trial.** S. E. Silverstone and M.
Nelson, "Food production and nutrition in Biosphere 2: results from the first
mission September 1991 to September 1993", *Advances in Space Research*
18(4-5):49-61, 1996, doi:10.1016/0273-1177(95)00861-8, abstract read through
PubMed's E-utilities
([PMID 11538814](https://pubmed.ncbi.nlm.nih.gov/11538814/)):

- "The initial test of the Biosphere 2 agricultural system was to provide a
  nutritionally adequate diet for eight crew members during a two year closure
  experiment, 1991-1993."
- "The 2000 m2 cropping area provided about 80 percent of overall nutritional
  needs during the two years."
- "the diet which averaged 2200 calories, 73 g. of protein and 32 g. of fat
  per person" and "The crew experienced 10-20 percent weight loss, most of
  which occurred in the first six months".
- "Overall, the agriculture and food processing required some 45% of the crew
  time."
- So: 250 m2 per person for about 80 percent of needs (computed). Scaled
  linearly to 100 percent, about 313 m2, which is Ecology Action's 3,403 sq ft
  (F1). The trial also shows the cost: hungry people and nearly half of all
  working time spent on food.

### B. Conventional agriculture

**F5. Ten US diet scenarios: 0.13 to 1.08 ha per person.** C. J. Peters,
J. Picardy, A. F. Darrouzet-Nardi, J. L. Wilkins, T. S. Griffin and G. W. Fick,
"Carrying capacity of U.S. agricultural land: Ten diet scenarios", *Elementa:
Science of the Anthropocene* 4:000116, issued 2016-07-22, abstract read at the
[Allegheny College repository](https://dspace.allegheny.edu/entities/publication/5f5990df-373d-46e1-9ec4-6793a5088cb5):

- "Annual per capita land requirements ranged from 0.13 to 1.08 ha person-1
  year-1 across the ten diet scenarios." That is 1,300 to 10,800 m2 (computed).
- This is conventional farmland at real yields, grazing included, not
  intensive beds. The diets are modelled scenarios, not surveyed ones: "The
  scenarios included two reference diets based on actual consumption and
  eight “Healthy Diet” scenarios that complied with nutritional
  recommendations but varied in the level of meat content."

**F6. World land per person (FAO data, via the World Bank).** World Bank World
Development Indicators API, `lastupdated` 2026-07-13, read at
[api.worldbank.org](https://api.worldbank.org/v2/country/WLD/indicator/AG.LND.ARBL.HA.PC?format=json&mrv=5):

- Indicator "Arable land (hectares per person)", World, 2023: "value":
  0.171679388898311. That is 1,717 m2 per person (computed). Its source note:
  "FAO electronic files and web site, Food and Agriculture Organization of the
  United Nations (FAO)".
- Indicator "Agricultural land (sq. km)", World, 2023: 47921707.067; and
  "Population, total", World, 2023: 8062923417. Divided, that is about 5,940
  m2 of agricultural land per person (computed). Most of it is grazing (F7).
- This is what the world actually uses, at today's diets and yields. It is
  not what a complete diet requires.

**F7. Our World in Data.** Hannah Ritchie and Max Roser, "Half of the world's
habitable land is used for agriculture", published 2019-11-11,
[ourworldindata.org/global-land-for-agriculture](https://ourworldindata.org/global-land-for-agriculture):
"In total, it is an area of 48 million square kilometers (km²)." and
"Croplands comprise one-third of agricultural land, and grazing land comprises
two-thirds." Hannah Ritchie, "If the world adopted a plant-based diet, we would
reduce global agricultural land use from 4 to 1 billion hectares", published
2021-03-04,
[ourworldindata.org/land-use-diets](https://ourworldindata.org/land-use-diets):
"our total agricultural land use would shrink from 4.1 billion hectares to 1
billion hectares." The article gives no per-person figure; 1 billion hectares
shared by about 8 billion people is roughly 1,250 m2 each (computed, and only
as rough as that population figure).

### C. Controlled-environment growing (lamps, hydroponics, CO2)

**F8. Wheat alone: 12 to 30 m2 per person.** F. B. Salisbury, B. Bugbee and
D. Bubenheim, "Wheat production in controlled environments", in *Controlled
Ecological Life Support System: Regenerative Life Support Systems in Space*,
NASA Ames Research Center, publication date 1987-09-01, abstract read at
[NTRS 19880002888](https://ntrs.nasa.gov/citations/19880002888):

- "With yields of 23 to 57 g/sq m/d of edible biomass, a minimum size for a
  CELSS would be between 12 and 30 sq m per person, utilizing about 600 W/sq m
  of electrical energy for artificial light."
- This is wheat only, the most area-efficient calorie crop under lamps, not a
  complete diet. 600 W per m2 for 12 to 30 m2 is 7 to 18 kW of lamps per
  person (computed).

**F9. NASA's planning range: about 50 m2 per person for a colony.** Raymond M.
Wheeler, NASA Kennedy Space Center, "Development of Bioregenerative Life
Support for Longer Missions: When Can Plants Begin to Contribute to Atmospheric
Management?", presentation slides, PDF created 2015-05-19, hosted at
[simoc.space](https://simoc.space/wp-content/uploads/2018/09/When-Can-Plants-Begin-to-Contribute-to-Atmospheric-Management-WHEELER.pdf):

- A slide headed "Role of Bioregenerative Components for Future Missions"
  gives plant growing area by stage: "~1-5 m 2 total" (short missions), "~10-25
  m 2 / person" (longer missions), "~50 m 2 / person" (autonomous colonies),
  credited to "Wheeler, 2004. Acta Hort."
- Oxygen alone needs less: "One Human's Oxygen from 11 m2 of Wheat !", credited
  to "Edeen and Barta. 1995. JSC No. 33636".

**F10. NASA's Baseline Values and Assumptions Document: 65.29 m2 per crew
member for its most-closed diet.** M. K. Ewert, T. T. Chen and C. D. Powell
(eds.), *Life Support Baseline Values and Assumptions Document*,
NASA/TP-2015-218570/REV2, February 2022,
[NTRS 20210024855](https://ntrs.nasa.gov/api/citations/20210024855/downloads/BVAD_2.15.22-final.pdf):

- Table 4-92, "Inedible Biomass Generation for Exploration Life Support Diets
  Based on Fresh Weight", has a growing-area column in "[m²/CM]" (square
  metres per crew member) for each of three diets. Its totals row reads
  "Total 1.35 0.07 19.50 4.29 65.29 6.66": growing area then inedible biomass
  for the "Diet Using Only ELS Salad Crops" (1.35 m2), the "Diet Using Salad
  and Carbohydrate Crops" (19.50 m2) and the "Diet Using All ELS Crops"
  (65.29 m2). The pairing was checked against the document's summary table,
  whose inedible-biomass values "0.07 4.29 6.66" match.
- What those diets cover, Section 4.5.7: the salad-and-carbohydrate diet
  "provides somewhere around half of the necessary mass through crops grown
  on-site"; the all-crops diet "uses a wide variety of species, and provides a
  high degree of closure", but "the resupply mass includes herbs and
  condiments" and "resupply items provide necessary nutrients that are not
  available in sufficient quantities within the grown biomass."
- The productivities behind the table are conservative: "The nominal rates are
  derived from testing within the Biomass Production Chamber (BPC) at Kennedy
  Space Center" and "These rates are lower partly because of the lower light
  levels".
- So 65.29 m2 is close to, but not quite, a complete diet, at measured (not
  record) chamber yields.

**F11. BIO-Plex: two growing chambers could not feed four people a balanced
diet.**

- Facility: T. O. Tri, "Bioregenerative Planetary Life Support Systems Test
  Complex (BIO-Plex): Test Mission Objectives and Facility Development", SAE
  1999-01-2186, published 1999-07-12,
  [saemobilus.sae.org/content/1999-01-2186](https://saemobilus.sae.org/content/1999-01-2186):
  chambers "capable of supporting test crews of four individuals for periods
  exceeding one year."
- Chamber size: D. J. Barta and J. M. Castillo, "Preliminary Designs of the
  Biomass Production System for the Bioregenerative Planetary Life Support
  Systems Test Complex", SAE 2001-01-2319, published 2001-07-09,
  [saemobilus.sae.org/content/2001-01-2319](https://saemobilus.sae.org/content/2001-01-2319):
  "The BPS will utilize two Biomass Production Chambers (BPC1 and BPC2)" and
  "In these designs the chamber will have 79 m2 of area for crop growth."
  The same abstract says the paper "gives a synopsis of designs of the
  Biomass Production System presented at a preliminary design review
  conducted August 3, 2000, emphasizing BPC1."
- The result: H. Jones, X. Kwauk and S. C. Mead, "Matching Crew Diet and Crop
  Food Production in BIO-Plex", publication date 2000-01-11 in the NTRS
  record, which also lists it under the "30th International Conference on
  Environmental Systems" (Toulouse, 10 to 13 July 2000),
  [NTRS 20010081946](https://ntrs.nasa.gov/citations/20010081946): "We can
  easily grow one-half the crew calories in one BIO-Plex Biomass Production
  Chamber (BPC) if we grow only the most productive crops (wheat, potato, and
  sweet potato)" but "We can not grow 95 percent of the crew calories in two
  BPCs at nominal productivity while growing a balanced diet."
- **No area per person is derived here.** The Jones abstract states no
  chamber area. The 79 m2 comes from designs reviewed in August 2000, after
  the Jones paper was written for the July conference, and the abstract says
  those designs emphasise the first chamber only. Nothing read shows that
  Jones assumed two 79 m2 chambers, so dividing 158 m2 by four people would
  pair two documents that may describe different chambers. The finding that
  stands is the qualitative one: two chambers, at nominal productivity, could
  not grow 95 percent of four people's calories in a balanced diet.
  BIO-Plex was never completed (secondary sources say it was shut down; that
  was not checked against a primary source).

**F12. Lunar Palace 1 (Yuegong-1): 30 m2 per person of stacked shelves, 73
percent of food.**

- The 370-day run: Y. Fu, Z. Yi, Y. Du, H. Liu, B. Xie and H. Liu,
  "Establishment of a closed artificial ecosystem to ensure human long-term
  survival on the moon", bioRxiv preprint doi:10.1101/2021.01.12.426282,
  "this version posted January 14, 2021" (not peer reviewed),
  [biorxiv.org](https://www.biorxiv.org/content/10.1101/2021.01.12.426282v1.full.pdf):
  - "Its internal cultivation device had a three-layer stereoscopic design,
    with a total planting area of 120 m2."
  - Crew of four at a time: "Eight volunteering crew members were divided into
    two groups", and the paper calls it "the current four-person 370-day
    experiment".
  - "The food regeneration rate in the system, calculated in dry weight, was
    73% throughout the experiment (Fig. S5). In fresh weight, the regeneration
    rate was 83% instead, with a 100% plant-based food regeneration rate."
  - Light: in plant cabin I "the lighting was continuous (24/0 h light/dark),
    with the light intensity of 300-800 [µ]mol m-2 s-1"; in cabin II, 12/12 h at
    "250-800 [µ]mol m-2 s-1".
  - Comparison systems in the same paragraph: "the four-person, 180-day closed
    experiment system (260 m2) and the Japanese two-person 28-day CEEF system
    (150 m2)", which reached 55 percent and "92% - 95%" respectively.
- The earlier 105-day run: Wikipedia, "Yuegong-1", last edited 2026-04-13,
  [en.wikipedia.org/wiki/Yuegong-1](https://en.wikipedia.org/wiki/Yuegong-1):
  "58m2 vegetation area of two cabins" and, with a crew of three, "55% of the
  food consumed is to be produced internally".
- So: 30 m2 of shelf per person (computed), on roughly 10 m2 of floor per
  person if the 120 m2 is three equal layers (computed, an assumption), gave
  73 percent of the food by dry weight, with animal protein from mealworms and
  the rest of the food and condiments brought in. Per head this is the best
  closed-system result found, and it ran on continuous electric light.

**F13. ESA MELiSSA: no complete-diet area published that we found.** D.
Garcia-Gragera et al. (including C. Lasseur, ESA-ESTEC), "Integration of
Nitrifying, Photosynthetic and Animal Compartments at the MELiSSA Pilot
Plant", *Frontiers in Astronomy and Space Sciences* 8:750616, published
2021-10-19,
[doi:10.3389/fspas.2021.750616](https://www.frontiersin.org/journals/astronomy-and-space-sciences/articles/10.3389/fspas.2021.750616/pdf):
the pilot plant's compartments "have been scaled-up to achieve the oxygen
production equivalent to the respiration needs of one human (0.84 kg·d-1)
(Wieland, 2005), with 20–40% concomitant production of edible material." Its
crew is rats, and its main oxygen producer is a cyanobacterium (*Limnospira
indica*) in a photobioreactor, not plants on shelves, so it gives no
plant-area-per-person figure for a full diet.

**F14. Vertical-farm wheat, for scale.** S. Asseng et al., "Wheat yield
potential in controlled-environment vertical farms", *PNAS*
117(32):19131-19135, published 2020-08-11 (online 2020-07-27),
[doi:10.1073/pnas.2002655117](https://doi.org/10.1073/pnas.2002655117), read
via PubMed and PubMed Central (PMC7430987):

- "wheat grown on a single hectare of land in a 10-layer indoor vertical
  facility could produce from 700 ± 40 t/ha (measured) to a maximum of 1,940 ±
  230 t/ha (estimated) of grain annually".
- The "measured" figure is one chamber experiment scaled up: an "Observed 70-d
  season indoor experiment with 20 h of 1,400 μmol/m2/s light daily (50
  MJ/m2/d) and 330 ppm atmospheric CO2 concentration, scaled up to 1 ha and
  multiplied by 5 harvests/y", then multiplied by ten layers.
- On cost: "more than one-half of current costs are for electricity powering
  the artificial lighting" and "it is unlikely to be economically competitive
  with current market prices."
- So one layer yields about 70 t/ha a year (computed, 700 divided by 10
  layers), about 20 times the world field average the paper quotes ("3.2
  t/ha"), at a light level several times full sunlight for 20 hours a day.

### Comparison in one table (computed from F1 to F14)

| Way of growing | m2 per person | What it covers | Source |
|---|---|---|---|
| Biointensive, 2019 staff answer | 232 | complete vegan diet + compost crops, intermediate yields, paths included, 2,400 kcal/day | F3 |
| Biointensive, 1978 claim | 260 | "as little as", complete vegetarian diet | F1 |
| Biosphere 2, measured | 250 for ~80% (313 scaled to 100%) | mostly vegetarian, crew lost weight | F4 |
| Biointensive, book figure | 372 | complete diet, intermediate yields, fertility sustained | F2 |
| Conventional, US vegan | 650 | fossil fuels available | F2 |
| Conventional, US diet scenarios | 1,300 to 10,800 | ten modelled diets | F5 |
| World arable land actually used | 1,717 | today's diets and yields | F6 |
| World agricultural land incl. grazing | about 5,940 | today's diets and yields | F6 |
| Lamps + CO2, wheat only | 12 to 30 | calories from one crop | F8 |
| Lunar Palace 365, stacked shelves | 30 | 73% of food by dry weight | F12 |
| Lamps, NASA colony planning | about 50 | autonomous colony | F9 |
| Lamps, NASA BVAD all-crops diet | 65.29 | high closure, some resupply | F10 |
| **Our docs today** | **700 to 1,000** | **no source** | self-sufficiency.md:82 |

BIO-Plex (F11) is left out of the table because no area per person could be
derived from what was read.

---

## Part 2: speeds and handling times

### Walking

**F15.** Wikipedia, "Preferred walking speed", last edited 2026-07-27,
[en.wikipedia.org/wiki/Preferred_walking_speed](https://en.wikipedia.org/wiki/Preferred_walking_speed):
"typically falling between 1.10 metres per second (4.0 km/h; 2.5 mph; 3.6
ft/s) and 1.65 metres per second (5.9 km/h; 3.7 mph; 5.4 ft/s)." (Already used
by the logistics design; restated here so the data file has it in one place.)

### Freight lifts and mine hoists

**F16. A facilities handbook.** APPA (the facilities management association),
"Elevator Systems", Body of Knowledge, no page date,
[appa.org/elevator-systems](https://www.appa.org/elevator-systems):

- Hydraulic lifts "typically operate at a maximum speed of 150 feet per minute
  (fpm)" (0.76 m/s, computed).
- Geared traction lifts "typically operate at speeds of 200 fpm to 500 fpm in
  passenger-, service-, and freight-elevator" applications (1.0 to 2.5 m/s,
  computed).

**F17. A manufacturer's freight planning guide.** TK Elevator, *Freight
Elevator Planning Guide* (Canada), "© 2022 TK Elevator Corporation",
[tkelevator.com](https://tkelevator.com/media/usa_canada/downloads_1/freight_elevator_planning_guide_ca_en.pdf):

- Hydraulic freight: "Speed feet per minute (fpm)" columns "50 (0.25m/s)", "75
  (0.38m/s)", "100 (0.5m/s)".
- "Traction Elevators for Freight": "100 (0.5m/s)", "150 (0.75m/s)", "200
  (1.0m/s)".
- Common capacities listed run from "2500 (1134 kg)" to "10,000 (4536 kg)"
  pounds.
- And the ceiling on size, TK Elevator, "Service Elevators vs. Freight
  Elevators" (2022, from the page address),
  [tkelevator.com](https://www.tkelevator.com/us-en/company/insights/service-elevator-freight-elevator-differences.html):
  "freight elevators can often carry loads up to 100,000 lbs."
- So a heavy freight lift in a building is slow: 0.25 to 1.0 m/s in the
  planning guide. APPA's 2.5 m/s top is for geared traction lifts generally,
  not specifically for heavy freight cars.

**F37. Mine hoists (added after review, so numbered last).** Jack de la
Vergne, *Hard Rock Miner's Handbook*, Edition 5, Stantec Consulting, "Edition
5 (CD/Web) - January 2014", chapter 13 "Drum Hoists", section 13.2 "Rules of
Thumb", page 114. Stantec's own download address
(`stantec.com/content/dam/stantec/files/PDFAssets/2014/Hard Rock Miner's Handbook Edition 5_3.pdf`)
refused the request with HTTP 403, so the handbook was read from a copy filed
as an exhibit with the Permanent Court of Arbitration,
[files.pca-cpa.org](https://files.pca-cpa.org/pcadocs/bi-c/1.%20Investors/2.%20Witness%20Statements%20and%20Expert%20Reports/B.%20Quantum/FTI%20Consulting%20Expert%20Report%20(Rosen)%20-%20Reply%20-%20Exhibits/Tab%2021%20-%20Stantec.%20Hard%20Rock%20Miners%20Handbook.pdf):

- The economic speed grows with the depth: "the following rule of thumb
  equation for the optimum economic speed for drum hoists, in which H is the
  hoisting distance. Optimum Speed (fpm) = 44H½ , where H is in feet Or,
  Optimum Speed (m/s) = 0.405 H½ , where H is in metres" (credited to Larry
  Cooper; H½ means the square root of H). At 100 m that is 0.405 x 10 =
  about 4.1 m/s (computed).
- The ceiling: "The maximum desirable speed for a double-drum hoist with fixed
  steel guides in the shaft is 18m/s (3,600 fpm)" (credited to Peter Collins).
- These are rules of thumb, not a standard: the handbook's disclaimer says
  "The content of each rule or tip is the expression and opinion of its
  author". They describe mine shaft hoists, not building freight lifts.

### Conveyors

**F18. Parcel conveyors.** Flexco, parcel-industry brochure X5016, dated
08/01/19 ("©2019 Flexible Steel Lacing Company"),
[documentlibrary.flexco.com](https://documentlibrary.flexco.com/X5016_enSG_4665_ParcelIndustryBrochure_080119.pdf):
"facilities have increased their belt speeds, with some belts moving at up to
180 meters per minute." That is 3.0 m/s (computed), as a top, not a typical.

**F19. The fastest bulk belts.** Siemens press release, 22 November 2016,
"Siemens provides eco-friendly speed boost" (Kaltim Prima Coal),
[press.siemens.com](https://press.siemens.com/sg/en/pressrelease/siemens-provides-eco-friendly-speed-boost-kaltim-prima-coal):
"the Melawan overland conveyor, which is a curved conveyor that runs at a
speed of 7.5 metres per second" and "The second conveyor belt, "OLC2," is a
straight conveyor belt that runs at a speed of 8.5 metres per second", over "a
total distance of 25 kilometres". "Their respective speeds makes them the
fastest two overland conveyor belts in the world." These are record overland
belts, not plant conveyors.

**F20. Airport baggage, for scale.** Airport Technology, 2008-03-26, on
Heathrow Terminal 5,
[airport-technology.com](https://www.airport-technology.com/?p=20120):
"Over 18km of conveyor belt, controlled by 118 computers servers able to
process 12,000 bags an hour" and "8km of destination coded vehicles, including
the fast-track system operating at 10m a second."

### Pneumatic tubes

**F21. Hospital tubes.** Vanessa Armstrong, MIT Technology Review, 2024-06-19,
[technologyreview.com](https://www.technologyreview.com/2024/06/19/1093446/pneumatic-tubes-hospitals/):
"The carriers or capsules, which can hold up to five pounds" (2.3 kg,
computed) "move through piping six inches in diameter" "at speeds of 18 to 24
feet per second, or roughly 12 to 16 miles per hour" (5.5 to 7.3 m/s,
computed). The article gives these as typical of hospital tube systems, and
the speed is a deliberate cap: "The carriers are limited to those speeds to
maintain specimen integrity." Penn Medicine's main system "runs over 12
miles of pipe and completes more than 6,000 transactions on an average day."

**F22. Hospital tube, door to door.** Sara Wykes, Stanford Medicine,
2010-01-11,
[med.stanford.edu](https://med.stanford.edu/news/all-news/2010/01/gone-with-the-wind-tubes-are-whisking-samples-across-hospital.html):
"Depending on the diameter of a tube, cylinders can reach speeds of up to 25
feet per second, about 18 miles per hour" (7.6 m/s, computed), and "a
container can cover the longest start-to-finish distance-1,500 feet-in less
than three minutes". 1,500 ft (457 m) in 180 s is an average above 2.5 m/s
(computed): door to door, a tube averages well under its top speed because of
routing, switching and slowing at stations.

**F23. A tube maker's general figures.** Air-Log (a German maker whose
"Applications" menu lists "Hospital", "Cash", "Industry" and "Sample
Transportation"), "Carriers", no page date,
[air-log.com/en/carriers.html](https://www.air-log.com/en/carriers.html):
"Depending on type and operating mode, our carriers can reach a speed of up to
11 meters per second (about 40 km/h) in a straight tube during transport", an
average of "about six to eight meters per second", and "Depending on the size
of the carrier, they can transport items with up to 5 kg."

**F24. Large-bore hospital tubes.** Aerocom's exhibitor entry for WHX Cape
Town 2025 (a health trade fair) on the German pavilion site (run for the
German economics ministry), dated only by the fair and the site's "© 2026",
[whx-cape-town.german-pavilion.com](https://whx-cape-town.german-pavilion.com/en/sites/exhibitors/123977):
"Pneumatic tube systems transport blood samples, blood bags, medications,
medical devices etc. from any department in a hospital to another. The tube
diameters of our logistic systems range from 110mm to 315mm. Materials
weighing nearly 28 kg can be transported at speeds of 8 meters per second."
So this is a hospital system too, a large-bore one, not an industrial one;
the 2.3 kg in F21 is a typical six-inch hospital tube's carrier, not a
ceiling for hospital tubes. This is a manufacturer's claim, not an independent
measurement.

### Automated guided vehicles and mobile robots

**F25. A trade magazine on typical speed.** Bob Trebilcock, *Modern Materials
Handling*, 2014-05-01,
[mmh.com](https://www.mmh.com/article/building_the_faster_safer_agv):
"But, at 60 meters per minute – about 2.2 miles per hour - you don't implement
AGVs for speed." (1.0 m/s, computed.) The faster figure is a question, not a
product: "But what if an AGV could safely travel at three times or more the
usual speed limit, say 200 meters per minute, or 7.5 miles per hour" (3.3 m/s,
computed). In the article's real-world testing "the system was limited...to
130 meters per minute, or just under 5 miles per hour" (2.2 m/s, computed).
Note for the logistics design: its "fast systems 3.3 m/s" reads that "what
if" as a shipped speed.

**F26. Current robots, by payload.** Mobile Industrial Robots specification
sheets, both dated "2023-06-28",
[MiR250](https://www.hartfiel.com/wp-content/uploads/2024/04/MiR250-Specs.pdf)
and [MiR1350](https://www.hartfiel.com/wp-content/uploads/2024/04/MiR1350-Specs.pdf)
(copies hosted by a distributor):

- MiR250: payload "250 kg | 551 lbs"; "Maximum speed (with maximum payload on
  a flat surface)" "2.0 m/s (7.2 km/h) | 6.6 ft/s (4.4 mph)".
- MiR1350: payload "1 350 kg | 2 976 lbs"; maximum speed "1.2 m/s (4.3 km/h) |
  3.9 ft/s (2.7 mph)".
- A heavier robot that is not slower: Rockwell Automation, *OTTO 1500 Spec
  Sheet*, document "OTTO-DS001F-EN-AUG2026" (August 2026), linked as "OTTO
  1500 Data Sheet" from [ottomotors.com/1500](https://ottomotors.com/1500/),
  [cdn.sanity.io](https://cdn.sanity.io/files/armc7p5y/production/65f37f30ec55c632329ba48e31cb7013ca96c80d.pdf):
  "Max. Capacity" "1,900 kg (4,190 lb)" "Includes payload and attachment, if
  any"; "Max. Speed" "2.0 m/sec (4.5 mph)". The sheet also says "Move your
  heaviest payloads at top speeds while safely navigating around people,
  equipment and tight turns."
- Two cautions on the OTTO figures. Unlike the MiR sheets, this sheet does
  not state the speed at full payload in its table; the full-load claim is
  in its sales text ("faster than any AMR on the market" appears beside it).
  And it is an "autonomous mobile robot (AMR)", which finds its own way,
  where the data row describes a cart "following floor markers". For speed
  and payload, though, it shows that 2.0 m/s carrying a tonne or more is a
  current product, not a stretch.

**F27. Warehouse drive units.** Wikipedia, "Amazon Robotics", last edited
2026-09-10,
[en.wikipedia.org/wiki/Amazon_Robotics](https://en.wikipedia.org/wiki/Amazon_Robotics):
"The maximum velocity of the robots was 1.3 metres per second (4.3 ft/s)." The
smaller model was "capable of lifting 1,000 pounds (450 kg)" and the larger
"capable of carrying a pallet with loads as heavy as 3,000 pounds (1,400 kg)."

### Light rail, metro, main-line rail and maglev

**F28. Metro average speeds, stops included.** "Automation speeds up metros",
from *Metro Report International*, reposted by London Reconnections on
2018-02-12,
[archive.londonreconnections.com/?p=19212](https://archive.londonreconnections.com/?p=19212)
(the logistics design dates the original 2018-02-07; that date was not
confirmed here):

- London Underground "trains reach an average speed of 33 km/h" with "a mean
  distance of about 1·25 km between them".
- Tokyo "an average speed of around 30 km/h".
- København "a scheduled average speed of about 35 km/h, with only 1 km
  between stations".
- Vancouver SkyTrain Expo Line "averages 45 km/h with 1·5 km between
  stations".
- Paris Line 1, after automation, "the average speed rose from 24·4 km/h to 30
  km/h".
- So 30 to 45 km/h (8.3 to 12.5 m/s, computed) door-closing to door-closing,
  whatever the top speed.

**F29. Light rail top speed.** Wikipedia, "Light rail", last edited
2026-08-26,
[en.wikipedia.org/wiki/Light_rail](https://en.wikipedia.org/wiki/Light_rail):
"The latest generation of LRVs is considerably larger and faster, typically 29
m (95 ft 1+3⁄4 in) long with a maximum speed of around 105 km/h (65.2 mph)."

**F30. United States track-class speed limits.** 49 CFR 213.9, "Classes of
track: operating speed limits", read in the eCFR point-in-time version for
2026-09-01 (last amended "85 FR 63388, Oct. 7, 2020"),
[ecfr.gov](https://www.ecfr.gov/current/title-49/subtitle-B/chapter-II/part-213/subpart-A/section-213.9),
"[In miles per hour]", freight then passenger:

- "Class 4 track 60 80" (freight 26.8 m/s, passenger 35.8 m/s, computed);
- "Class 5 track 80 90" (freight 35.8 m/s, passenger 40.2 m/s, computed).
- The higher classes are in 49 CFR 213.307, "Classes of track: operating
  speed limits", in subpart G, read the same way (eCFR point-in-time version
  for 2026-09-01, last amended "78 FR 16104, Mar. 13, 2013"),
  [ecfr.gov](https://www.ecfr.gov/current/title-49/subtitle-B/chapter-II/part-213/subpart-G/section-213.307):
  "Class 6 track" "110 m.p.h." (49.2 m/s, 177 km/h, computed), "Class 7
  track" "125 m.p.h.", "Class 8 track" "160 m.p.h." and "Class 9 track" "220
  m.p.h.", with the note "Operating speeds in excess of 125 m.p.h. are
  authorized by this part only in conjunction with FRA regulatory approval".
  Freight runs at these speeds only under conditions: "Freight may be
  transported at passenger train speeds if the following conditions are
  met".

**F31. Maglev in service and under construction.**

- Wikipedia, "Shanghai maglev train", last edited 2026-06-04,
  [en.wikipedia.org/wiki/Shanghai_maglev_train](https://en.wikipedia.org/wiki/Shanghai_maglev_train):
  "The top operational commercial speed of the Shanghai maglev was 431 km/h
  (268 mph)" until "its speed reduction in May 2021"; it now "has a maximum
  cruising speed of 300 km/h (186 mph)". "The journey takes 8 minutes and 10
  seconds to complete the distance of 30 km", an average of about 220 km/h
  (computed).
- JR Central, *Integrated Report 2025*, Chuo Shinkansen section,
  [global.jr-central.co.jp](https://global.jr-central.co.jp/en/company/ir/annualreport/_pdf/annualreport2025-12.pdf):
  "Maximum design speed" "505 km/h"; the system "enables the vehicle to
  levitate about 10 cm" and makes it "possible to travel at an ultra high speed
  of 500 km/h in a stable manner"; and "JR Central records the world speed
  record for a manned rail vehicle at 603 km/h."
- The other kind of maglev, the one Shanghai uses: Wikipedia, "Transrapid",
  last edited 2026-09-03,
  [en.wikipedia.org/wiki/Transrapid](https://en.wikipedia.org/wiki/Transrapid).
  It levitates "using the attractive magnetic force between two linear arrays
  of electromagnetic coils", and "the dipole gap remains nominally constant
  at 10 millimetres (0.39 in). When levitated, the maglev vehicle has about
  15 centimetres (5.9 in) of clearance above the guideway surface." Its
  latest version, "the 2007-built Transrapid 09, is designed for a cruising
  speed of 505 km/h", and "In 2002, the first commercial implementation was
  completed – the Shanghai Maglev Train". So a 10 mm magnet gap belongs to
  this design, and a 10 cm one to JR Central's.

### Pipelines and a space elevator

**F32. A long-distance slurry pipeline.** N. T. Cowper Snr, N. T. Cowper Jnr
and A. D. Thomas, "Iron Ore Slurry Pipelines: Past, Present and Future",
Slurry Systems Engineering Pty Limited, undated (the text says the line "has
operated continuously for over 43 years" since 1967, so it was written in
about 2010 or later),
[swapoff.org](https://swapoff.org/files/allan-thomas-papers/20%20IronOre%20SlurryPipelines,%20past,%20present%20and%20future.pdf):
for the Savage River magnetite line, "An optimum pumping concentration of 60%
solids by weight and an operating velocity of 1.68 m/s were predicted", with
"a non-standard pipe diameter of 9 inches". The line has run since "October
26, 1967". One pipeline is one data point, and "predicted" is a design
figure, not a measured one.

**F33. Space elevator climbers.** Bradley C. Edwards, *The Space Elevator*,
NIAC Phase I final report (the file name says 2000; the date was not found in
the text read),
[nss.org](https://NSS.org/wp-content/uploads/2017/07/2000-Space-Elevator-NIAC-phase1.pdf):
the first climbers deploy cable "at high velocity (up to 200 km/hr)", and
"With deployment speeds of 200 km/hr and our proposed spool size" the spool
turns "at less than 1000 RPM." 200 km/h is 55.6 m/s (computed). No space
elevator exists; this is a design study, the most cited one.

### Loading and handling times

**F34. Truck loading and unloading: 2 hours, as the industry's working
definition.** J. Dunn, J. Hickman, S. Soccolich and R. Hanowski, *Driver
Detention Times in Commercial Motor Vehicle Operations*, 2014, a study for the
US Federal Motor Carrier Safety Administration, abstract read at
[vtechworks.lib.vt.edu/handle/10919/55062](https://vtechworks.lib.vt.edu/handle/10919/55062):

- "Although there is currently no standard definition, the industry commonly
  defines detention time as “any time drivers have to wait beyond 2 hours,
  which is the average time it takes to load or unload their cargo."" So the
  2 hours is the industry's assumption, quoted by the study, not something
  the study measured.
- What the study did measure: drivers met detention on about "1 in every 10
  stops for an average duration of 1.4 hours", and at such a stop "he/she was
  loading/unloading at that delivery location for 3.4 hours in total."

**F35. A train's stop at a busy station: 30 to 50 seconds.** Transit
Cooperative Research Program, *Transit Capacity and Quality of Service
Manual*, 3rd Edition (TCRP Report 165), Chapter 8 "Rail Transit Capacity",
2013 (the file was last modified 2013-07-31),
[onlinepubs.trb.org](https://onlinepubs.trb.org/onlinepubs/tcrp/tcrp_rpt_165ch-08.pdf),
page 8-52: "Existing rail transit systems operating at or close to capacity
have median station dwell times over the peak hour that range from 30 to 50 s
with occasional exceptional situations-such as the heavy peak hour mixed flow
at NYCT's Grand Central Station of more than 60s." And: "Most station dwell
times in Exhibit 8-37 fit into the 35 to 45 s range". These are passenger
stops; no freight equivalent was found.

**F36. Other handling figures already above:** a metro's stop is folded into
its 30 to 45 km/h average (F28); a hospital tube's switching and slowing is
folded into its door-to-door 2.5 m/s (F22); Heathrow's baggage system
processes "12,000 bags an hour" (F20); a MiR250 needs "1.95 m | 6.4 ft" to
reach full speed from a marker (F26 sheet, "Minimum distance to achieve maximum
speed").

### `data/transportation.ron` row by row

Every row that carries a speed (m/s converted to km/h where that helps,
computed). "Within" means inside the range the sources above report.

| Row (line) | Data | Sources say | Verdict |
|---|---|---|---|
| `dirt_path` speed (19) | 2.0 m/s, carts up to 1.0 t (23) | walking 1.10 to 1.65 m/s (F15) | not an outlier: it is a speed limit on a path that carries carts, not a walking pace (no cart speed was researched) |
| `gravel_road` (30) | 8.3 m/s (30 km/h) | not researched | posted limits are local law, not physics |
| `paved_road` (41) | 13.9 m/s (50 km/h) | not researched | as above |
| `highway` (52) | 31.3 m/s (113 km/h, 70 mph) | not researched | as above |
| `bridge` (64) | 16.7 m/s (60 km/h) | not researched | as above |
| `tunnel` (76) | 22.2 m/s (80 km/h) | not researched | as above |
| `light_rail` (95) | 22.2 m/s (80 km/h) | modern LRVs to "around 105 km/h" (F29); metros average 30 to 45 km/h with stops (F28) | within, as a top speed |
| `heavy_rail` (106) | 44.4 m/s (160 km/h) | FRA Class 5 passenger 90 mph = 40.2 m/s; Class 6 110 mph = 49.2 m/s (F30) | within, as a passenger top speed on Class 6 track; freight runs that fast only under the conditions F30 quotes |
| `maglev` (117) | 138.9 m/s (500 km/h) | JR Central 500 km/h, design 505 (F31); Transrapid 09 designed for 505 (F31); Shanghai now 300 | within |
| `maglev` gap (122) | 10 mm | Transrapid's gap "nominally constant at 10 millimetres"; JR Central's maglev levitates "about 10 cm" (F31) | within: matches the Transrapid type, the one Shanghai uses; do not "correct" it to 100 mm |
| `cargo_rail` (128) | 27.8 m/s (100 km/h) | FRA freight 60 mph (Class 4) to 80 mph (Class 5) = 26.8 to 35.8 m/s (F30) | within |
| `space_elevator` climb (176) | 200 m/s (720 km/h) | Edwards "up to 200 km/hr" = 55.6 m/s (F33) | 3.6 times the source; looks like km/h written as m/s |
| `conveyor_belt` (219) | 1.5 m/s | parcel belts up to 3.0 m/s (F18); record overland belts 7.5 to 8.5 m/s (F19) | within |
| `pneumatic_tube` (229 to 231) | 8.0 m/s, 10 kg, 200 mm | typical six-inch hospital tubes 5.5 to 7.6 m/s, 5 lb, 6 in (F21, F22); a maker's carriers up to 11 m/s, 6 to 8 average, 5 kg (F23); large-bore hospital system 110 to 315 mm, nearly 28 kg at 8 m/s (F24) | within: matches a large-bore hospital system; bigger than a typical hospital tube |
| `cargo_elevator` (240 to 242) | 2.0 m/s, 5 t, 100 m travel, "for multi-level facilities and mine shafts" (243) | building freight traction 0.5 to 1.0 m/s for 1.1 to 4.5 t (F17); geared traction generally 1.0 to 2.5 m/s (F16); mine drum hoist economic speed 0.405 x square root of depth, about 4.1 m/s at 100 m, up to 18 m/s (F37) | depends on the use: above the building freight guide, below the mine hoist rule of thumb |
| `automated_guided_vehicle` (278, 279) | 2.0 m/s, 1.0 t | 1,900 kg robot rated 2.0 m/s (OTTO 1500, F26); 250 kg robot 2.0 m/s and 1,350 kg robot 1.2 m/s (MiR, F26); 450 to 1,400 kg drive units 1.3 m/s (F27); typical AGV 1.0 m/s (F25) | within: a current product carries more at the same speed; many heavy robots are slower |
| `pipe_transport` (288) | 3.0 m/s | Savage River design velocity 1.68 m/s (F32) | nearly double the one line read |

Rows not checked, and why: `throughput_kg_per_s` on every logistics row (no
source read gives throughput in comparable terms), the `airlock`
`cycle_time_s` at line 157, the `orbital_tether`, and the signalling rows. They
are outside the two questions asked.

---

## What is still unknown, and what would settle it

- **Where 700 to 1,000 m2 came from.** No outside source read gives it. The
  repository history narrows it down but does not settle it:
  - `git log -S` shows the line entered `self-sufficiency.md` in commit
    605db94bb (2026-06-13, "indoor-garden light-cap finding"), with no source
    in the commit or the line.
  - The repository's oldest gardening note,
    [`docs/history/knowledge-gardening.md`](../../history/knowledge-gardening.md)
    (in the initial commit, 2026-01-16, itself unsourced), gives biointensive
    as "4,000 sq ft + paths (total ~8,000 sq ft)". 8,000 sq ft is 743 m2
    (computed), at the bottom of the 700 to 1,000 range. Ecology Action's own
    2019 answer (F3) adds paths as 2,000 sq ft of beds becoming 2,500 sq ft,
    a quarter more, not double. So the likeliest trail is an unsourced
    doubling for paths, carried forward. **That is a reading of our own
    files, not a finding about the world.**
  - Settled by: asking whoever wrote that note, or accepting that it has no
    source and replacing it (no cost).
- **How much more area beginners need.** Ecology Action's figures are at
  "intermediate" yields; the book's beginning-yield figure was not read.
  Settled by: the yield tables in *How to Grow More Vegetables*, 8th ed.,
  pages 40 to 41 and the master charts (a library copy, an hour).
- **What calorie level the book figure assumes.** Settled for the 2019
  figure: Ecology Action's staff use "2,400 calories per day" (F3). Still
  open for the book's 4,000 sq ft (F2): page 214 states no calorie level.
  Settled by: the book's pages 40 to 41, or Ecology Action's *One Basic
  Mexican Diet* or *Diet Design* booklets (purchase, small cost).
- **How much the greenhouse glass and the latitude change the sun-lit
  figure.** Ecology Action's trials are in coastal California; Biosphere 2 was
  in Arizona under glass. The game's ship has no sun at all unless it pipes
  light in. Settled by: a light-scaled model, for which the
  [crop daily light integrals findings](2026-09-26-crop-daily-light-integrals.md)
  already hold the per-crop light needs.
- **BIO-Plex's fate and the CEEF details.** Secondary sources say BIO-Plex was
  shut down; the primary record was not read. The Japanese CEEF figures came
  from the Lunar Palace paper, not from CEEF's own reports. Settled by: NTRS
  and JAXA repository searches (an hour, free).
- **A handling time for one freight transfer inside a building or ship**
  (robot to lift, lift to rail, rail to depot). Nothing read measures it
  directly; the passenger dwell times in F35 and the truck dock time in F34
  are the nearest. Settled by: a materials-handling text on transfer times,
  or CIBSE Guide D for lift door and loading times (purchase), or timing a
  real hospital or warehouse transfer.
- **Freight lift speeds above 1.0 m/s for heavy cars in buildings**, from
  another manufacturer, to settle whether F17 or F16 is the better planning
  figure. Settled by: Otis or KONE freight planning guides (free, an hour).
  For mine hoists, a hoist maker's catalogue would confirm the handbook's
  rules of thumb (F37) with real installations (free, an hour).
- **Slurry velocities from more than one line.** Settled by: two or three more
  pipeline papers (Samarco, Antamina; free abstracts, some paywalled).
- **Throughput figures** for tubes, conveyors and lifts in the same units as
  the data file. Not attempted.

## Sources that could not be reached

- `https://www.biorxiv.org/content/10.1101/2021.01.12.426282v1.full-text`
  and the `.full.pdf` address: HTTP 429 to the fetch tool. The PDF was read
  through a direct download instead.
- `https://online.ucpress.edu/elementa/article/doi/10.12952/journal.elementa.000116/...`
  and `https://doaj.org/article/fa9a57f7dce74ac98c0a4371fefa45ad`: HTTP 403.
  Wanted: the full Peters et al. paper, to see which diet sits at 0.13 ha. The
  abstract was read at the Allegheny College repository instead.
- `https://pubmed.ncbi.nlm.nih.gov/11538814/`: a cookie wall. The abstract was
  read through NCBI's E-utilities instead.
- SAE full texts for 1999-01-2186 and 2001-01-2319, and the abstract of
  2001-01-2320 ("Estimating Plant Growth Area With The Biomass Production
  Chamber Sizing Model"): paywalled. Wanted: BIO-Plex's planned crop mix and
  per-person area.
- `https://www.kuka.com/-/media/swisslog-healthcare/documents/products-and-services/transport/translogic-pts/pts-210-translogic-pneumatic-tube-system-carriers-and-accessories.pdf`:
  HTTP 404. Wanted: Swisslog's carrier payloads and speeds (a search summary
  claimed "up to 12 lbs" at "up to 25 feet per second"; not used, since it
  could not be read).
- `https://africahealth.german-pavilion.com/en/sites/exhibitors/99053`: HTTP
  404; the same Aerocom text was read on the Cape Town pavilion page (F24).
- `https://melissafoundation.org/download/765`: HTTP 404, and
  `https://ddd.uab.cat/record/37901`: connection dropped. Wanted: MELiSSA's
  higher-plant chamber area and any full-diet projection.
- `https://www.fmcsa.dot.gov/research-and-analysis/impact-driver-detention-time-safety-and-operations`:
  HTTP 403. Wanted: FMCSA's own summary of the detention study.
- `https://www.ecfr.gov/...section-213.9` (web page): redirected to a bot
  check. Read through the eCFR versioner API instead, as was section 213.307
  (F30).
- `https://www.stantec.com/content/dam/stantec/files/PDFAssets/2014/Hard%20Rock%20Miner's%20Handbook%20Edition%205_3.pdf`:
  HTTP 403. The same handbook was read from the arbitration-court copy linked
  in F37 instead.
- `https://growbiointensive.org/Enewsletter/Summer2019/caloriesFAQ.html`:
  HTTP 403 to a plain scripted request; it answered one sent with ordinary
  browser headers (F3).
- `https://canadianminingjournal.com/featured-article/overland-conveyors-go-the-distance`
  (2023-05-01): read, but it gives no speeds.
- The World Bank *Container Port Performance Index 2020* (2021): read, but its
  crane-moves-per-hour averages are only in a chart, not in the text, so no
  number from it is quoted. Wanted: a sourced quay-crane handling rate.
- Fenner Dunlop's conveyor handbook belt-speed table: only on document-sharing
  sites; not read.

---

## What it means for this project

### Certain (from the sources)

- The 700 to 1,000 m2 per person in `self-sufficiency.md:82`,
  `homestead-solo-design.md:170` and `data/home_outline.json:107` has no
  source that this research could find, and it is about two to four times
  each Ecology Action figure used (232 to 372 m2, F1 to F3; F2 and F3 state
  intermediate yields, F1 states none; the beginning-yield figures were not
  read).
- At the sourced figures, the 1,156 m2 garden room would feed about 3.1
  people at the book figure (372 m2) and up to 5.0 at the 2019 figure (232
  m2), if it were sun-lit soil beds at intermediate yields (computed). The
  homestead's 2,270 m2 would feed about 6 to 10 (computed). The logistics
  design's "about 2 to 9 people" already reflects part of this.
- The arithmetic in `data/home_outline.json:107` rests on the unsourced area.
  At that line's own 2,400 kcal per day, which is also the figure Ecology
  Action's staff use (F3), the sourced 232 to 372 m2 give 6.5 to 10.3
  kcal/m2/day, not 2.4 to 3.4 (computed). Biosphere 2 achieved about 7.0
  (2,200 kcal x 0.8 / 250 m2, computed, assuming the 80 percent applies to
  calories). The line's "2 to 3x biointensive" therefore does not survive.
  What replaces it is less certain: the biointensive figures are field
  yields from coastal California, the greenhouse's 7.2 kcal/m2/day is for an
  unlit greenhouse in western Washington, and how latitude and glass change
  the sun-lit figure is still open (above). So the two are not a like-for-like
  pair, and "about equal" is a judgement, not a finding.
- Lamp-lit, CO2-enriched chambers need between about a twelfth and a quarter
  of the area of sun-lit beds per person (30 to 65 m2 against 232 to 372,
  computed), and in every source they are paid for in electricity (F8, F14).
  The canopy-cap argument in `self-sufficiency.md` (light, not floor, is the
  limit) is supported; its number is not.
- In `data/transportation.ron`, two rows are above the sources read: the
  space elevator climb (200 m/s against a sourced 55.6) and the slurry pipe
  (3.0 m/s against one line's design velocity of 1.68). The AGV (F26: a
  1,900 kg robot rated 2.0 m/s), the pneumatic tube (F24: a large-bore
  hospital system, nearly 28 kg at 8 m/s), heavy rail (F30: Class 6 track,
  110 mph) and the maglev gap (F31: Transrapid's 10 mm) are not. The dirt
  path is unverified: no cart speed was sourced, and the row describes itself
  as a footpath, while walking runs 1.10 to 1.65 m/s against its 2.0. The cargo lift is above
  a building freight lift guide and below a mine hoist rule of thumb (F17,
  F37), and the row covers both.
- The logistics design's "Parcel up to 2.3 kg" class is a typical six-inch
  hospital tube's carrier (F21), not a ceiling: a large-bore hospital system
  carries nearly 28 kg (F24).
- The logistics design's claim that the AGV and tube rows exceed real
  systems is wrong for both (F24, F26).

### Judgement calls (ours; a reader may weigh them differently)

- **Use two food-area numbers, keyed to the light.** For sun-lit soil beds,
  use **372 m2 per person** (Ecology Action's book figure, F2): it is the
  publisher's own planning number, it includes the compost crops a closed
  homestead must grow, and Biosphere 2 (F4) lands close to it with real
  people. Keep 232 m2 (F3) as the "expert grower" end. For lamp-lit
  hydroponic chambers with CO2, use **about 50 m2 per person** (F9) as the
  design target and **65 m2** (F10) as the cautious figure, and charge the
  lamps to the energy loop as the game already does. Lunar Palace (F12) shows
  30 m2 of stacked shelf can reach 73 percent, which suits a "skilled,
  stacked" upgrade rather than the default.
- **Scale by skill and diet, not by a single constant.** A conventional
  mixed diet needs 4 to 30 times more land (F2, F5); a beginner needs more
  than "intermediate" (not yet sourced). The game's food loop teaches more if
  the area follows the diet the player chooses.
- **Re-check the "one person" conclusion in `homestead-solo-design.md`** with
  these figures before it drives more data. Whether the garden room is lit by
  the sun or by lamps decides which figure applies, and on a mothership it is
  presumably lamps (or piped light), which makes the area small and the power
  bill the real cap.
- **Treat the home greenhouse and sun-lit biointensive beds as roughly
  comparable per square metre** (7.2 against 6.5 to 10.3 kcal/m2/day) until
  the latitude question is settled, rather than claiming either is several
  times the other.
- **Correct `data/transportation.ron`:** set the climb speed to about 55.6
  m/s (or state a different design and its source); lower the slurry speed
  toward 1.7 m/s until more lines are read. Leave the AGV at 2.0 m/s and 1
  t (F26), the heavy rail at 160 km/h (F30) and the maglev gap at 10 mm
  (F31). For the cargo lift, either split it into a building freight lift
  (1.0 m/s, F17) and a mine hoist (about 0.405 x the square root of the
  shaft depth in m/s, about 4 m/s at 100 m, F37), or leave it alone, since
  2.0 m/s sits between the two. Keep the tube row, call it a large-bore
  hospital tube, and if a smaller one is wanted add a typical hospital size
  (7 m/s, 2.3 kg, 150 mm, F21, F22). Add a source link and date to every
  row, as the logistics design already requires of world facts.
- **Plan travel with averages that include stops** (F28), as the logistics
  design already decided, and carry handling time per node in data, but mark
  it unsourced until the open item above is closed. The 2 hour truck figure
  (F34) is a ground-transport dock, not a ship's internal transfer, and
  should not be copied across without a reason.

## Corrections made after review (2026-10-03)

An independent review on the research date, before this file was first
committed, found these errors in the first draft. They were corrected the
same day:

- **AGV:** the draft called a 1 tonne AGV at 2.0 m/s too fast and proposed
  splitting the row. Rockwell's OTTO 1500 (1,900 kg, 2.0 m/s) refutes that;
  added to F26, and the split was dropped.
- **2019 answer (F3):** the 2,500 sq ft passage is by Ecology Action staff,
  not John Jeavons, and the same page gives the calorie level (2,400 a day),
  which closed half of an open question.
- **Large tubes (F24):** the 28 kg system is a hospital system, not an
  industrial one.
- **Cargo lift:** the draft set it to 1.0 m/s without noticing the row also
  covers mine shafts; the mine hoist rule of thumb was added (F37).
- **Heavy rail:** the draft called it both "within" and "unconfirmed"; US
  Class 6 track (F30) settles it as within.
- **Maglev gap:** the draft said 10 mm did not match; it matches the
  Transrapid type (F31).
- **Dirt path:** taken out of the outliers, by the draft's own test (the row
  carries carts).
- **Wording:** "every figure Ecology Action publishes" narrowed to the
  figures read; "roughly a tenth" corrected to "a twelfth to a quarter"; the
  greenhouse comparison marked as not like for like; the truck "2 hours"
  marked as a working definition; "real US diets" called scenarios; the
  BIO-Plex "about 40 m2 per person" withdrawn, since it paired two documents
  that may describe different chambers (F11).

**Again: this is a reading of public sources by people who are not
agronomists or engineers, dated 2026-10-03.** If a source changes, write a new
dated finding that links back to this one.
