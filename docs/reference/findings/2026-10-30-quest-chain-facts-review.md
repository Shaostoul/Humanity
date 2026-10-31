# Survival quest chain facts: the refutation pass

**Research date: 2026-10-30.** Every source below was opened and read on that date, and every
verdict is as of that date.

> **Not a doctor, nurse, food scientist or lawyer.** This is a check of what published sources say,
> written so a game can teach from them. It is not medical advice. In an emergency, call the
> emergency number for where you are.

## The question

Does each of the 61 facts in section 3 of
[`docs/design/quest-chains.md`](../../design/quest-chains.md) (W1 to W28, F1 to F15, H1 to H8, A1
to A4, C1 to C3, M1, M2, R1) say what its primary source says, is that source the current one, and
does any other primary source contradict it?

## Short answer

**38 confirmed, 19 corrected, 1 refuted, 3 unconfirmed.** The ones that change what the game would
teach:

- **W21, refuted in part.** Ready.gov says distillation removes "most other chemicals". University
  of Nebraska Extension and Connecticut's Department of Public Health say chemicals that boil near
  or below water's boiling point (volatile solvents, VOCs such as benzene and toluene, some
  pesticides) vaporise with the water and come back in the distillate. Distillation must not clear
  solvents in the game, not even "partly". For fuel, CDC (W6) already says water containing it
  cannot be made safe by boiling; that a still also fails on fuel is our inference from its volatile
  parts, not a source's words, and the game takes the cautious side.
- **W15 and D5, corrected.** The 2025 review's 4.5-log virus figure for sand filters comes from
  laboratory studies (4.7 from 72 estimates); under typical household operation it was 1.7 log from
  5 estimates. A household sand filter does not make water safe from viruses.
- **W9, corrected.** CDC and EPA both say to double the bleach for cloudy, murky, coloured or very
  cold water. The two doses that looked different (CDC 8 drops, Ready.gov 1/8 teaspoon per gallon)
  agree: CDC's own table puts 8 drops at "½ mL" and "a little less than ⅛ teaspoon".
- **A1 and W23, corrected.** The 2018 table was read correctly (oxygen 0.816 kg in, carbon dioxide
  1.04 kg out, 2.5 kg potable water), but it is not the current edition: NASA's February 2022
  revision gives 0.895 kg oxygen, 1.085 kg carbon dioxide and 3.217 kg potable water for the same
  82 kg crew member. The game's 2004 figures, scaled by food energy as the game does, agree with the
  2022 edition within 1%.
- **M2 and D7, corrected.** The other bodies were found: the American Red Cross's Scientific
  Advisory Council (2019) says at least 10 minutes and ideally 20, and never beyond 40; the NHS
  (2026) says 20 minutes. The game should teach 20 minutes under cool running water.
- **F15, corrected.** Missouri's own wild carrot page says the purple centre floret is "usually"
  there, so its absence does not mean poison hemlock; common water hemlock, which Missouri calls
  probably the most toxic plant in North America, is the other deadly lookalike.
- **H8 and a new disagreement D10.** MedlinePlus says an alert person with a heat emergency may sip
  a drink, and nobody vomiting or unconscious gets anything by mouth; CDC's sheet says nothing to
  drink in heat stroke. The game's line stays with CDC for heat stroke and adds MedlinePlus's rule.
- **Settled questions from 3.6:** hand sanitiser on dirty or greasy hands (C2, now confirmed from
  CDC's own page), iodine's medical limits (found on three CDC pages), the 165 °F poultry jerky
  figure (F8, now from the University of Kentucky's extension service citing USDA FSIS), and the
  heat stroke temperature (D6: MedlinePlus gives above 104 °F, 40 °C, matching the game).
- **Still unknown:** a solar still's daily yield (W22); the WHO excreta guidelines (C3) and the
  crop gas-exchange citations (A4) were not re-read.

## How this pass was done

Each fact's cited source was opened again, the quotation in the design was checked word for word,
and the source was searched for anything the design left out that changes the lesson (a condition,
an exception, a newer edition). Where a fact was on the list of the shakiest, a second primary
source was looked for. The default verdict was UNCONFIRMED until a source clearly said it.

- **CONFIRMED**: the source says it, the quotation is exact (or was corrected in punctuation only),
  and no source read contradicts it.
- **CORRECTED**: the source supports a different or fuller wording; the new wording is given.
- **REFUTED**: a primary source contradicts the claim as written; the replacement is given.
- **UNCONFIRMED**: no primary source was read for it this pass.

US federal works (CDC, NASA, EPA, FDA, OSHA, NIOSH, NOAA, FEMA, DHS) are public domain and quoted
as written. Copyrighted sources (MedlinePlus's licensed encyclopedia, the National Academies, the
University of Georgia's NCHFP, university extension services, the Missouri Department of
Conservation, the Red Cross, the NHS, the journal article) are quoted only in short fragments and
otherwise described, with enough detail to find the passage. PDFs were turned into text with
`pdftotext` 4.00 (`-raw` for tables, which keeps each table row together).

## Verdicts at a glance

| Id | Verdict | What changed |
|---|---|---|
| W1 | CONFIRMED | |
| W2 | CONFIRMED | |
| W3 | CONFIRMED | |
| W4 | CONFIRMED | |
| W5 | CONFIRMED | |
| W6 | CONFIRMED | EPA (2026) agrees |
| W7 | CONFIRMED | |
| W8 | CONFIRMED | EPA pairs 5,000 ft with "1,000 meters" (noted) |
| W9 | CORRECTED | double for cloudy or very cold water; the measures; Ready.gov's smell check |
| W10 | CONFIRMED | |
| W11 | CORRECTED | the Yellow Book: kills at practical doses and contact times of several hours |
| W12 | CONFIRMED | |
| W13 | CONFIRMED | |
| W14 | CORRECTED | plain ceramic filters 1.3 log for viruses; almost all lab data |
| W15 | CORRECTED | 4.5 log is lab; 1.7 log in typical operation |
| W16 | CONFIRMED | |
| W17 | CORRECTED | chemicals from the roof too (asbestos, lead, copper) |
| W18 | CONFIRMED | |
| W19 | CONFIRMED | |
| W20 | CONFIRMED | |
| W21 | REFUTED (in part) | distillation does not remove chemicals that boil with the water |
| W22 | UNCONFIRMED | no yield from a reviewed source |
| W23 | CORRECTED | NASA's 2022 edition: 2.00 kg drinking plus 0.5 kg food water |
| W24 | CONFIRMED | quotation punctuation corrected |
| W25 | CONFIRMED | |
| W26 | CORRECTED | CDC says drinking, cooking and brushing teeth; more for some people |
| W27 | CONFIRMED | |
| W28 | CORRECTED | cover and shake, not swish |
| F1 | CONFIRMED | |
| F2 | CONFIRMED | |
| F3 | CORRECTED | "fish with fins"; the °C figures are our conversion |
| F4 | CONFIRMED | |
| F5 | CONFIRMED | |
| F6 | CONFIRMED | the data fix in 3.5 landed in BUG-216 |
| F7 | CONFIRMED | |
| F8 | CORRECTED | 165 °F poultry now sourced; USDA heats before drying |
| F9 | CONFIRMED | |
| F10 | CORRECTED | CDC's own words on the most common cause; figs and seafood |
| F11 | CONFIRMED | |
| F12 | CORRECTED | damaged or cracked containers; never taste |
| F13 | CONFIRMED | |
| F14 | CONFIRMED | quotation punctuation corrected |
| F15 | CORRECTED | the floret is "usually" there; water hemlock |
| H1 | CONFIRMED | |
| H2 | CORRECTED | no drink for an unconscious person |
| H3 | CONFIRMED | |
| H4 | CORRECTED | every bedroom, outside each sleeping area, every level |
| H5 | CONFIRMED | |
| H6 | CONFIRMED | |
| H7 | CONFIRMED | MedlinePlus differs on drinks (D10) |
| H8 | CORRECTED | nothing by mouth for anyone vomiting or unconscious |
| A1 | CORRECTED | the 2022 edition supersedes 2018 |
| A2 | CONFIRMED | |
| A3 | CONFIRMED | |
| A4 | UNCONFIRMED | not re-read (the design says so) |
| C1 | CONFIRMED | |
| C2 | CONFIRMED | now from CDC's own page |
| C3 | UNCONFIRMED | WHO's PDF still refused |
| M1 | CORRECTED | arm or leg; padding; Stop the Bleed's poster agrees |
| M2 | CORRECTED | 20 minutes; never past 40; no cold-water soak for a severe burn |
| R1 | CONFIRMED | |

## The findings, fact by fact

### Water

#### W1

**How much water a person takes in a day. CONFIRMED.**
National Academies, "Report Sets Dietary Intake Levels for Water, Salt, and Potassium...",
https://www.nationalacademies.org/news/report-sets-dietary-intake-levels-for-water-salt-and-potassium-to-maintain-health-and-reduce-chronic-disease-risk
(last update 2004-02-11). All four quoted fragments are on the page as written, including "men
average approximately 3.7 liters" (followed by "(125 ounces) daily"). The page also calls these
"adequate intake levels" and says the panel did not set an upper limit for water, which bears on the
water-intoxication question in 3.6.

#### W2

**Water counts from food. CONFIRMED.** Same page: "the other 20 percent is derived from food."

#### W3

**The signs of drying out. CONFIRMED.**
MedlinePlus, "Dehydration", https://medlineplus.gov/ency/article/000982.htm (review date
2025-07-01). The three quoted list items are exact. The page's words are "Rapid heartbeat" and
"Rapid breathing"; the design's "fast" is our wording, which is fine outside quotation marks.

#### W4

**Drink more when you sweat. CONFIRMED.** W1's page: "those who are very physically active or who
live in hot climates may need to consume more water."

#### W5

**Clear cloudy water before treating it. CONFIRMED.**
CDC, "How to Make Water Safe in an Emergency", https://www.cdc.gov/water-emergency/about/index.html
(dated 2024-09-19). "Bring clear water to a rolling boil for 1 minute (at elevations above 6,500
feet, boil for 3 minutes)." and "If the water is cloudy, first filter it through a clean cloth, paper
towel, or coffee filter." and "Another option is to let it settle."

#### W6

**Chemicals are a different problem. CONFIRMED.** W5's page: "You cannot make water containing
fuel, toxic chemicals, or radioactive materials safe by boiling or disinfecting it."
A second source agrees: EPA, "Emergency Disinfection of Drinking Water",
https://www.epa.gov/ground-water-and-drinking-water/emergency-disinfection-drinking-water (last
updated 2026-02-24): "boiling or disinfection will not destroy other contaminants, such as heavy
metals, salts, and most other chemicals."

#### W7

**Boiling kills every kind of germ. CONFIRMED.**
CDC, "A Guide to Drinking Water Treatment and Sanitation for Backcountry and Travel Use",
https://www.cdc.gov/healthywater/drinking/travel/backcountry_water_treatment.html (dated
2025-01-30): "Boiling kills all types of germs, including viruses, bacteria, and parasites (like
Cryptosporidium and Giardia)." The CDC Yellow Book chapter (W12) adds: "All organisms that may cause
illness from drinking water are killed within seconds at boiling temperature."

#### W8

**How long to boil. CONFIRMED.** W7's page: "At elevations above 6,500 feet, boil for 3 minutes."
EPA's page (W6): "At altitudes above 5,000 feet (1,000 meters), boil water for three minutes." Note
that EPA's own figures do not match (5,000 feet is about 1,524 m, our arithmetic); the existing
how-to's choice of 1,000 m is the cautious reading of all three numbers.

#### W9

**Household bleach. CORRECTED.**
The quoted fragments are exact, but the fact left out a condition both government sources give.
- W5's CDC page: "If the water is cloudy, murky, colored, or very cold, add double the amount of
  bleach listed below." Its table for 5% to 9% bleach gives, for 1 gallon, "Add 8 drops of bleach",
  "Add ½ mL of bleach" and "Add a little less than ⅛ teaspoon"; for 1 quart or liter, "Add 2 drops of
  bleach" or "Add 0.1 mL of bleach". For 1% bleach, 10 drops per quart or liter. "Let the water stand
  for at least 30 minutes before you drink it."
- EPA (W6's page, 2026-02-24): 6% bleach, 8 drops per gallon; 8.25% bleach, 6 drops per gallon;
  "Double the amount of bleach if the water is cloudy, colored, or very cold."; "Stir and let stand
  for 30 minutes."; "The water should have a slight chlorine odor."
- Ready.gov, "Water", https://www.ready.gov/water (last updated 2021-02-22): "Add 1/8 teaspoon of
  bleach per gallon of water, stir and let stand for 30 minutes."; "If it doesn't, then repeat the
  dosage and let stand another 15 minutes."; "If it still does not smell of chlorine, discard it and
  find another source of water."

New wording: "Plain unscented household bleach of 5% to 9% sodium hypochlorite: 2 drops per litre or
quart of clear water, 8 drops (about ½ mL) per gallon; twice that if the water is cloudy, murky,
coloured or very cold; stir, then wait at least 30 minutes before drinking. For 1% bleach, 10 drops
per litre. The water should smell slightly of chlorine; if it does not, dose again and wait 15
minutes more, and if it still does not, do not drink it."

#### W10

**Bleach and iodine do not kill Cryptosporidium. CONFIRMED.** Both quotations are exact. The CDC
Yellow Book chapter (W12) agrees and goes further: Cryptosporidium "is poorly inactivated by
chlorine- or iodine-based disinfection at practical concentrations", "even with extended contact
times". Three CDC pages also give iodine's limits, which 3.6 asked for: W5's page, "If you are
pregnant, have thyroid problems, or have a hypersensitivity to iodine, avoid drinking water
disinfected with iodine." (the backcountry page has the same sentence) and "no one should drink
water disinfected with iodine for more than a few weeks at a time."

#### W11

**Chlorine dioxide and parasites. CORRECTED.** Both quotations are exact, and a third CDC source
settles most of D1.
- W5's page (2024): "Chlorine dioxide tablets will kill Cryptosporidium as long as you follow the
  instructions correctly."
- W7's page (2025): "Chlorine dioxide tablets can kill the parasite Giardia and are somewhat
  effective at killing Cryptosporidium." The same page: "Follow the instructions on any products you
  use to make sure they work properly to remove germs."
- CDC Yellow Book 2026, "Water Disinfection for Travelers",
  https://www.cdc.gov/yellow-book/hcp/preparing-international-travelers/water-disinfection-for-travelers.html
  (dated 2025-04-23): ClO2 kills waterborne pathogens "including Cryptosporidium oocysts, at
  practical doses and contact times"; the tablets and drops "require prolonged contact time of
  several hours"; and "relatively few data are available on the use of these products for different
  water conditions".

New wording: "Chlorine dioxide tablets kill Giardia, and kill Cryptosporidium only when the product's
instructions are followed, which means waiting several hours; CDC's pages differ on how reliably
(one says they kill it, another 'somewhat effective'), and there is little data on cloudy or cold
water. The game takes the cautious reading: partly, and only after the full wait."

#### W12

**Pore sizes. CONFIRMED.** W5's page: "Must have an absolute pore size of 1 micron or smaller to
remove parasites (such as Giardia or Cryptosporidium)."; "Must have an absolute pore (hole) size of
0.3 micron or smaller to remove bacteria"; "Will not remove viruses". W7's page: "A filter with an
absolute pore (hole) size of 0.3 micron or smaller will remove bacteria and parasites, but not
viruses." The Yellow Book's ultrafilter sentence is exact.

#### W13

**Filter, then disinfect. CONFIRMED.** W7's page: "The next best option is to filter water and then
disinfect the filtered water."

#### W14

**Clay pot filters. CORRECTED.**
- CDC (with USAID), "Preventing Diarrheal Disease in Developing Countries: Proven Household Water
  Treatment Options", https://stacks.cdc.gov/view/cdc/51530, PDF
  https://stacks.cdc.gov/view/cdc/51530/cdc_51530_DS1.pdf (January 2010). The quotation is exact:
  "Drawbacks include unknown effectiveness against viruses, lack of residual protection that can lead
  to recontamination". The 60-70% diarrhoea reduction it gives is "in commercially manufactured
  filters in conjunction with safe storage".
- Rao, Wells, Reynolds, Yoo, Kowalsky, DeFrance, Linden and Brown, "Systematic Review of the
  Microbiological Performance of Household Water Treatment Technologies", Environ. Sci. Technol.
  59(41):21776-21789, https://pmc.ncbi.nlm.nih.gov/articles/PMC12550818/ (accepted 2025-01-24,
  issue 2025-10-21, PMID 40138253). Its Table 1, mean log reduction for viruses: porous ceramic
  without additives 1.3 (1.0-1.6) from 56 point estimates, 54 of them under laboratory conditions;
  ceramic with metal oxides or silver 1.9 (1.4-2.5) from 52 point estimates, 51 in the laboratory
  and one under typical operation at 1.22. (The table's study counts span each technology's rows;
  the per-row counts are point estimates, and lab plus typical add up to them.) The authors warn
  that the with-and-without comparison "should be interpreted with caution".

New wording: "CDC lists ceramic pot filters among household treatments shown to cut diarrhoea, with
unknown effectiveness against viruses, no lasting protection, the need to keep filter and container
clean, and slow flow. The 2025 review found they take out viruses poorly: a mean of 1.3 log (about
95%) for plain ceramic and 1.9 log with silver or metal-oxide additives, almost all from laboratory
tests. A clay filter does not make water safe from viruses."

#### W15

**Sand filters. CORRECTED.** The CDC quotation is exact ("Benefits include turbidity removal, high
flow rate, ease of use, long-life"). The review's sentence is quoted correctly but the design drew
the wrong lesson from it. The authors call the result unexpected ("Unexpectedly, GMF technology
overall performed better for viral microbes"), and their Table 1 splits it: granular media
(biosand and slow sand) filters, viruses, 4.5 (4.0-5.0) overall from 77 point estimates, made of 4.7
(4.2-5.2) from 72 laboratory point estimates and **1.7 (1.1-2.3) from 5 point estimates under
typical operating conditions**; bacteria 1.9 overall and 1.6 in typical operation, the lowest mean
of any technology for bacteria. The review also says, in general, that laboratory testing "yielded
higher LRVs" than field or typical operation.

New wording: "CDC lists the household slow sand filter: it clears cloudiness, flows well and lasts,
with unknown effectiveness against viruses and the same recontamination risk. The 2025 review found
high virus removal in the laboratory (4.7 log) but only 1.7 log in the few measurements under
typical household use, and the lowest bacteria removal of any method (1.9 log). A sand filter
clears cloudiness and some germs; the water still needs disinfecting."

#### W16

**Charcoal traps but does not kill. CONFIRMED.** The Yellow Book sentence is exact.

#### W17

**Rainwater. CORRECTED.**
CDC, "Collecting Rainwater and Your Health: An Overview",
https://www.cdc.gov/drinking-water/about/collecting-rainwater-and-your-health-an-overview.html
(dated 2024-07-23). The three quotations are exact. The design left out the chemicals: "Once
rainwater falls, chemicals like asbestos, lead, or copper can get into the water." and "If your
rainwater contains germs or chemicals, treat it to remove those germs or chemicals." The page also
says to ask the local health department which germs and chemicals to test for.

New wording: "Rainwater is not safe to drink as it falls off a roof: germs (bird droppings, for one)
and chemicals that get into it once it falls (asbestos, lead or copper) wash in. A first flush diverter
sends the first rain of a storm away from the store. If you drink, cook or bathe with it, test it
regularly for germs and chemicals and treat it for what is found; killing germs does not take out
lead."

#### W18

**Test it rather than guess. CONFIRMED.** The quotation is exact (W17's page). What to test for: ask
the local health department, per the same page; what a home kit detects is still open (3.6).

#### W19

**Sunlight (SODIS). CONFIRMED.** All three Yellow Book quotations are exact; the page adds that the
bottles are "transparent bottles (e.g., clear plastic beverage bottles) laid on their side".

#### W20

**UV light needs clear water. CONFIRMED.** Both quotations from W7's page are exact.

#### W21

**Distillation. REFUTED (the chemicals clause); the procedure is confirmed.**
- Ready.gov (W9's page, 2021): the quotations are exact, including "as well as heavy metals, salts
  and most other chemicals." This is the claim refuted below.
- University of Nebraska-Lincoln Extension, Dvorak and Skipton, "Drinking Water Treatment:
  Distillation", G1493, https://extensionpubs.unl.edu/publication/g1493/html/view (revised December
  2013): pesticides, volatile solvents and VOCs such as benzene and toluene with boiling points close
  to or below water's "will vaporize along with the water as it is boiled in the distiller", and are
  not completely removed unless another process is used before condensation; gas vents, fractional
  columns or an activated carbon filter can reduce them, and without those, throwing away the first
  half litre removes them only to some degree.
- Connecticut Department of Public Health, "Distillation treatment systems for private drinking
  water systems",
  https://portal.ct.gov/dph/knowledge-base/articles/environmental-health/drinking-water/distillation-treatment
  (dated 2026-06-17): distillation is "not effective at removing most volatile organic compounds",
  which can vaporise with the steam and recontaminate the water, and should be removed before
  distilling.

New wording: "Boil the water and collect only the vapour that condenses: with a cup hung under an
upturned pot lid, boil for 20 minutes; the water that drips into the cup is distilled. Distillation
removes germs, salts and heavy metals. It does not remove chemicals that boil off with the water,
such as solvents and some pesticides: they condense back into the cup. Never count on a pot still
for water with fuel or solvents in it (CDC already says boiling cannot make fuel-contaminated water
safe, W6)."

#### W22

**A solar still's yield. UNCONFIRMED.** One more search found no peer-reviewed or extension source
with a daily yield. A July 2025 medRxiv preprint (not peer reviewed), "Improvised Water Procurement in
Wilderness Medicine: A Comparative Review of Yield, Energy Cost, and Field Suitability",
https://www.medrxiv.org/content/10.1101/2025.07.10.25331307v1, gives roughly 0.3 to 0.6 L a day; it
is a lead to its own cited sources, not a source. No number goes into the game.

#### W23

**The space station recovers 98% of its water. CORRECTED (the NASA baseline sentence).**
- NASA, "NASA Achieves Water Recovery Milestone on International Space Station",
  https://www.nasa.gov/missions/station/iss-research/nasa-achieves-water-recovery-milestone-on-international-space-station
  (published 2023-06-20, last updated 2026-03-02). All three quotations are exact; the gallon
  sentence continues "such as brushing teeth". The Brine Processor Assembly "runs it through a
  special membrane technology, then blows warm, dry air over the brine to evaporate the water."
- The baseline sentence cites the January 2018 edition (A1). The current edition is Ewert, Chen and
  Powell, "Life Support Baseline Values and Assumptions Document", NASA/TP-2015-218570/REV2
  (February 2022, NTRS distribution 2022-02-14), https://ntrs.nasa.gov/citations/20210024855, Table
  3-31: "Potable Water Content kg/CM-d 3.217", explained as "0.5 kg used for food preparation, 2.00
  kg for drinking water, and an additional 0.717 kg added due to increased perspiration as a result
  of the increased exercise profile".

New wording for the last sentence: "NASA's life support baseline (2022 edition) gives each crew
member 2.00 kg of drinking water and 0.5 kg for preparing food a day, plus 0.717 kg more for the
sweat of a daily exercise regime (3.217 kg in all); the 2018 edition gave 2.5 kg."

#### W24

**Reverse osmosis. CONFIRMED** (quotation punctuation corrected).
EPA WaterSense, "Point-of-Use Reverse Osmosis Systems",
https://www.epa.gov/watersense/point-use-reverse-osmosis-systems (last updated 2026-08-21). The
sentence continues after "membrane" with a comma, so the quotation is "RO is the process by which
pressure forces water through a semi-permeable membrane," not a full sentence with a full stop. The
list it gives is "lead, volatile organic compounds (VOCs), PFAS, arsenic, bacteria, and viruses",
which the systems "can potentially remove". The W7 and Yellow Book quotations are exact.

#### W25

**RO wastes water. CONFIRMED.** Both EPA fragments are exact.

#### W26

**How much to store. CORRECTED.**
CDC, "How to Create an Emergency Water Supply",
https://www.cdc.gov/water-emergency/about/how-to-create-and-store-an-emergency-water-supply.html
(dated 2025-06-27). Both quotations are exact. CDC does not say "for drinking and sanitation" (that
is Ready.gov's "for drinking and sanitation"); it says "You can use this water during an emergency
for drinking, cooking, brushing teeth, and other uses." and "Consider storing more water than this
for pregnant women, people who are sick, pets, or if you live in a hot climate."

New wording: "At least 1 gallon (3.8 L) per person per day for 3 days, for drinking, cooking and
brushing teeth (CDC; Ready.gov says 'for several days', for drinking and sanitation); a 2-week supply
if possible; more for pregnant women, people who are sick, pets, or a hot climate."

#### W27

**How to keep it. CONFIRMED.** All three CDC quotations are exact, including "(50°F –70°F)". The page
also says "Keep containers away from direct sunlight." and that FDA-approved food-grade containers
are best. Ready.gov's six months is for water "that has not been commercially bottled"; CDC says to
observe the expiration date for store-bought water.

#### W28

**Sanitising a container. CORRECTED.** The quotations are exact. CDC's steps are: wash with soap and
rinse; make the solution ("Use bleach that contains 5%–9% sodium hypochlorite."); "Cover the
container tightly and shake it well."; wait at least 30 seconds and pour it out; "Let the empty
container air-dry before use."

New wording: "Wash with soap and rinse; mix 1 teaspoon of unscented household bleach (5% to 9%) in 1
quart of water in the container; cover it tightly and shake it well; wait at least 30 seconds; pour
it out; let it air-dry."

### Food

#### F1

**2,000 kcal is a guide. CONFIRMED.** FDA (content current as of 2024-03-05): both quotations exact.

#### F2

**The four steps, and rinsing produce. CONFIRMED.** CDC, "Preventing Food Poisoning",
https://www.cdc.gov/food-safety/prevention/index.html (dated 2025-11-24): "Follow the four main steps
to food safety: clean, separate, cook, and chill."; "Rinse fresh fruits and vegetables under running
water."; "Wash for at least 20 seconds with soap and warm or cold water."

#### F3

**Safe internal temperatures. CORRECTED (wording).** The CDC page loads and carries the list
(FoodSafety.gov and FSIS were not attempted, as both refuse automated reading): "All poultry,
including ground chicken and turkey": "165°F"; "Ground meats, such as beef and pork": "160°F"; "Whole
cuts of beef, veal, lamb, and pork, including fresh ham": "145°F (then allow the meat to rest for 3
minutes before carving or eating)"; "Fish with fins": "145°F or cook until the flesh is opaque and
separates easily with a fork"; "Leftovers and casseroles": "165°F". CDC gives no Celsius figures.

New wording: "Poultry 165 °F (74 °C); ground meat 160 °F (71 °C); whole cuts of beef, veal, lamb and
pork 145 °F (63 °C), then 3 minutes' rest; fish with fins 145 °F, or until the flesh is opaque and
flakes with a fork; leftovers and casseroles 165 °F. (Celsius figures are our conversion; CDC gives
Fahrenheit only.)"

#### F4

**Two hours out, one hour in the heat. CONFIRMED.** F2's page: "Never leave perishable food out for
more than 2 hours (or 1 hour if exposed to temperatures above 90°F)." and the "Danger Zone" between
40°F and 140°F.

#### F5

**Fridge and freezer. CONFIRMED.** FDA, "Are You Storing Food Safely?" (content current as of
2023-01-18): all three quotations exact; "Appliance thermometers are the best way of knowing these
temperatures and are generally inexpensive." CDC's F2 page agrees: "Keep your refrigerator at 40°F or
below and your freezer at 0°F or below."

#### F6

**The game's storage zones. CONFIRMED.** A pointer to F5, not a separate fact. The data fix 3.5 asks
for has landed: commit a135204d3 (BUG-216, 2026-10-30) changed the `cold` zone's description in
`data/food_system.ron` to say a real refrigerator should hold 4 °C (40 °F) or below; the band still
runs to 8 °C as a spoilage-rate band.

#### F7

**Drying at 140 °F. CONFIRMED.** NCHFP, "Jerky", https://nchfp.uga.edu/how/dry/jerky (no page date;
extracted from So Easy to Preserve, 6th ed. 2014, a book reviewed and updated in 2020): racks go in
a dehydrator or oven "preheated to 140ºF".

#### F8

**Jerky: heat it before or after drying. CORRECTED (the poultry figure is now sourced).**
- NCHFP (F7's page): the meat's internal temperature must reach 160 ºF, "necessary to eliminate
  disease-causing bacteria such as E. coli O157:H7, if present", by heating the strips in marinade
  before drying or in a 275 ºF oven after. It does not mention poultry.
- University of Kentucky Cooperative Extension, Bastin and Norris, "Jerky Safety", FCS3-594,
  https://publications.ca.uky.edu/sites/publications.ca.uky.edu/files/FCS3594.pdf (issued March
  2017; its reference is USDA FSIS, "Jerky and Food Safety", 2016): USDA recommends "heating meat to
  160°F and poultry to 165°F before the dehydrating process" (the PDF's degree signs extract as an
  unmapped character), and the dehydrator should then hold 130 to 140 °F; sun or shade drying is
  not suitable for meat.

New wording: "Heat the meat to an internal 160 °F (71 °C), poultry to 165 °F (74 °C), because drying
alone may not kill E. coli O157:H7 or Salmonella. USDA heats it before drying; NCHFP also allows
heating the dried strips in a 275 °F oven afterwards. Then dry at 130 to 140 °F; never sun-dry meat."

#### F9

**Sauerkraut. CONFIRMED.** NCHFP, "Sauerkraut" (no page date; adapted from the USDA Complete Guide,
revised 2015): all three quotations exact; the salt is canning or pickling salt; above 75 ºF kraut may
become soft; the weight is a plate and weights or a brine-filled bag.

#### F10

**Low-acid food only in a pressure canner. CORRECTED (wording).**
CDC, "Home-Canned Foods", https://www.cdc.gov/botulism/prevention/home-canned-foods.html (dated
2024-04-25). The three quotations are exact. CDC's own words: "Low-acid foods have a pH higher than
4.6."; "These foods include all fresh vegetables, figs, meats, poultry, fish, seafood, and some
tomatoes."; "Home-canned vegetables, which are low-acid foods, are the most common cause of botulism
outbreaks in the United States."; "Do not use a boiling water canner for low-acid foods because it
will not protect against botulism."

New wording: "Low-acid foods (pH above 4.6: all fresh vegetables, figs, meats, poultry, fish, seafood
and some tomatoes) must be canned in a pressure canner, the only recommended method; a boiling-water
canner does not protect against botulism. Home-canned vegetables are the most common cause of
botulism outbreaks in the US. You cannot see, smell or taste the toxin. Use only recipes that follow
the USDA guide."

#### F11

**The pH line and the canner's temperature. CONFIRMED.** NCHFP, "Ensuring Safe Canned Foods" (no
page date; adapted from the USDA Complete Guide, Agriculture Information Bulletin No. 539, revised
2015): all three quotations exact; the 240 to 250 ºF is "attainable with pressure canners operated at
10 to 15 PSIG", and the time needed "ranges from 20 to 100 minutes". For the how-to, not the quest:
the page says sea-level process times may fail at 1,000 feet or more, and CDC (F10's page) says to
make sure the canner's gauge is accurate (county extension offices check them).

#### F12

**Throw out a bad jar. CORRECTED.** F10's page lists: "The container is leaking, bulging, or
swollen"; "The container looks damaged, cracked, or abnormal"; "The container spurts liquid or foam
when opened"; "The food is discolored, moldy, or smells bad"; then "If in doubt, throw it out! Never
taste food to see if it's safe."

New wording: "Throw out a container that leaks, bulges or is swollen, looks damaged, cracked or
abnormal, or spurts liquid or foam when opened, and food that is discoloured, mouldy or smells bad.
Never taste it to find out. If in doubt, throw it out."

#### F13

**Vitamin C. CONFIRMED.** CDC, National Report on Biochemical Indicators, "Vitamin C" (2026),
https://www.cdc.gov/nutrition-report/media/pdfs/2026/06/Water-Soluble-Vitamin-C-508.pdf: "scurvy,
which can occur if intake is below 10 mg/d for many weeks"; "For men, this equates to 90 mg/d, with
75 mg being the appropriate daily amount for women"; "people who smoke require an additional 35 mg/d";
"The most abundant dietary sources of vitamin C are orange juice, grapefruit juice, peaches, sweet
red peppers, and papayas". MedlinePlus, "Scurvy" (review 2025-07-03): its signs include general
weakness, anemia and gum disease, as quoted.

#### F14

**Wild mushrooms. CONFIRMED** (quotation punctuation corrected). CDC MMWR 75(20):258-263,
https://www.cdc.gov/mmwr/volumes/75/wr/mm7520a2.htm (2026-05-28). Both quoted sentences continue past
the quoted words, so the quotations end without a full stop: "Edible and toxic Amanita species can
be difficult to distinguish based on appearance alone" and "Amatoxins are not denatured by cooking or
other methods of food preparation". The delay is "often occurring >6 hours after ingestion". Of the
39 patients, three needed liver transplants and four died; the report's posters advised against
foraging wild mushrooms.

#### F15

**Poison hemlock and wild carrot. CORRECTED.** This is the lookalike field where the last round
failed, so it was read against two pages of the same publisher and one extension page.
- Missouri Department of Conservation, "Poison Hemlock",
  https://mdc.mo.gov/discover-nature/field-guide/poison-hemlock (no page date; site copyright 2026).
  Both design quotations are exact. It describes the stems as smooth, often with a white waxy
  coating, and spotted or mottled with purple; the foliage's smell as unpleasant and musty; "The
  sap can cause blistering and welts in contact with skin."; and says Queen Anne's lace's stems are
  hairy without purple markings. It names other lookalikes too: common water hemlock (which it
  calls probably the most toxic plant in North America), hedge parsley, sweet cicely, anise root and
  yarrow. On florets it says Queen Anne's lace "has a purple floret at the center of each flower
  cluster".
- Missouri Department of Conservation, "Queen Anne's Lace (Wild Carrot)",
  https://mdc.mo.gov/discover-nature/field-guide/queen-annes-lace-wild-carrot (no page date): "The
  central floret of the entire cluster is usually purple." So the poison hemlock page's "has" is too
  strong, by its own publisher's other page.
- Penn State Extension, Bosak, "Poison Hemlock Identification",
  https://extension.psu.edu/poison-hemlock-identification (no date): wild carrot's stems are densely
  hairy and poison hemlock's are not; wear gloves to avoid skin contact when handling hemlock.

New wording: "Eating any part of poison hemlock can kill, and its sap can blister skin, so handle it
only in gloves. Its stems are smooth, often waxy, and spotted or mottled with purple; wild carrot's
are hairy without purple marks. Wild carrot's flower cluster usually has one purple floret in the
centre and poison hemlock's florets are all white, but a missing purple floret does not mean
hemlock. Poison hemlock's leaves smell musty. Common water hemlock, probably the most toxic plant in
North America, is another lookalike. The state's advice: if you are not experienced at identifying
plants, treat every wild member of the carrot family as possibly deadly."

### Warmth, air, cleanliness, first aid and rest

#### H1

**Hypothermia. CONFIRMED.** CDC, "Preventing Hypothermia",
https://www.cdc.gov/winter-weather/prevention/index.html (dated 2026-08-25): the adult signs as
listed, and "If it is below 95° F, get medical attention immediately!" The game's 35 °C matches.

#### H2

**When help is not near. CORRECTED.** H1's page. The three quotations are exact. Left out: "Do not try
to give beverages to an unconscious person." and "You can also use skin-to-skin contact under loose,
dry layers of blankets, clothing, towels, or sheets." Once the temperature rises, keep the person dry
and wrapped, head and neck included, and get medical attention as soon as possible.

New wording: "A warm room; wet clothing off; warm the centre of the body (chest, neck, head, groin)
first, with an electric blanket or skin to skin under loose dry layers; warm drinks for someone awake
but no alcohol, and never a drink for someone unconscious; CPR even if the person seems dead; then
medical help as soon as possible."

#### H3

**A fire extinguisher. CONFIRMED.** OSHA eTool (no date shown): both quotations exact; "Back away
from an extinguished fire in case it flames up again."; "Evacuate immediately if the extinguisher is
empty and the fire is not out."; "Evacuate immediately if the fire progresses beyond the incipient
stage."

#### H4

**Smoke alarms. CORRECTED (wording).** US Fire Administration, "Smoke alarms",
https://www.usfa.fema.gov/prevention/home-fires/prepare-for-fire/smoke-alarms/ (last reviewed
2026-10-13). The three quotations are exact. The placement is smoke alarms "in every bedroom, outside
each separate sleeping area, and on every level of a home", the basement included, in the page's
words; "Test smoke alarms every month and replace 9-volt smoke alarm batteries at least once every
year."; "Choose interconnected smoke alarms so that when one sounds, they all sound."

New wording: "Test every month; one in every bedroom, outside each sleeping area, and on every level,
the basement included; interconnected if possible; replace 9-volt batteries at least yearly; replace
the alarm every 10 years."

#### H5

**Carbon monoxide. CONFIRMED.** CDC, "Carbon Monoxide (CO) Poisoning Basics",
https://www.cdc.gov/carbon-monoxide/about/index.html (updated 2026-10-26): both quotations exact; the
symptom list is "headache, dizziness, weakness, upset stomach, vomiting, chest pain, and confusion".

#### H6

**Preventing it. CONFIRMED.** H5's page: all three quotations exact. Also: batteries checked "when you
change the time on your clocks each spring and fall", detectors replaced per the maker or "every 5
years", and "Never use a generator inside your home or garage, even if doors and windows are open."

#### H7

**Heat stroke and heat exhaustion. CONFIRMED.** CDC, "Heat-Related Illnesses", sheet CS280226,
https://www.cdc.gov/natural-disasters/media/pdfs/2026/09/Heat_Related_Illness-NEWLOGO.pdf (served
from a 2026 path; the file's metadata says 2017-07-20). Read as text: all four quotations exact. Two
other sources bear on it, recorded as D6 and D10 below.

#### H8

**Heat stroke and the hydration chain. CORRECTED.** MedlinePlus, "Heat emergencies",
https://medlineplus.gov/ency/article/000056.htm (review 2025-10-14), first aid for heat emergencies:
if the person is alert, give a beverage to sip, such as a sports drink, or cool water; give nothing
by mouth, not even salted drinks, "if the person is vomiting or unconscious".

New wording: "H7 meets the hydration chain: in heat stroke you give the person nothing to drink
(CDC), and nobody who is vomiting or unconscious gets anything by mouth (MedlinePlus). So the Water
chain's 'drink more in the heat' (W4) never appears without H7 beside it in the warmth chain's text."

#### A1

**What a person breathes. CORRECTED (a newer edition).**
- The 2018 reading is right. Anderson, Ewert and Keener, NASA/TP-2015-218570/REV1 (January 2018),
  https://ntrs.nasa.gov/api/citations/20180001338/downloads/20180001338.pdf, page 63, Table 3-33,
  extracted in content order so each row stays whole: "- m Carbon Dioxide Load kg/CM-d 1.04 (1)",
  "+ m Oxygen Consumed kg/CM-d 0.816 (1)", "+ m Potable Water Consumed 78 kg/CM-d 2.5 (1)" (78 is a
  footnote: "This value includes drinking water and water used to hydrate food and drinks"). A
  stoichiometry check confirms the rows: with the table's respiratory quotient of 0.92, 0.816 kg of
  oxygen (25.5 mol) gives 23.5 mol, 1.03 kg, of carbon dioxide; the reverse would mean a quotient of
  0.57, which a human body does not run at (our arithmetic). The quoted sentence carries a footnote
  marker: "for a standard 82 kg crewmember with a respiratory quotient 74 of 0.92 during intravehicular
  activities".
- It is superseded. Ewert, Chen and Powell, NASA/TP-2015-218570/REV2 (February 2022),
  https://ntrs.nasa.gov/citations/20210024855, Table 3-31: "Respiratory Quotient 0.860", "Carbon
  Dioxide Load kg/CM-d 1.085", "Oxygen Consumed kg/CM-d 0.895", "Food Consumed; Energy Content
  MJ/CM-d 12.778", for "a standard 82 kg crewmember" whose balance "includes a rigorous daily exercise
  regimen which includes 30 minutes of aerobic and 60 minutes of resistance training". The game's
  own `data/life_support.ron` already quotes this edition's section 3.3.5 for its scaling rule.
- The game's figures are the August 2004 edition, Hanford, NASA/CR-2004-208941 (read from
  EverySpec's copy, https://everyspec.com/NASA/NASA-General/NASA_CR-2004-208941_30300/), Table 3.3.6:
  "Overall Body Mass kg 70.0", "Respiratory Quotient 0.869", "Carbon Dioxide Load kg/CM-d 0.998",
  "Oxygen Consumed kg/CM-d 0.835", "Food Consumed; Energy Content MJ/CM-d 11.82". The game's citation
  of it is accurate.
- Which is right for what (our arithmetic): the game scales breathing by food energy. Per MJ of food
  the 2004 edition gives 0.0706 kg oxygen and 0.0844 kg carbon dioxide; the 2022 edition gives 0.0700
  and 0.0849, within 1%. The 2018 edition is the outlier (0.0648 and 0.0826 per MJ of food). So the
  game's ratios are sound; the absolute numbers a player reads should be the 2022 edition's.

New wording: "NASA's life support baseline (2022 edition) for an 82 kg crew member who exercises
daily: 0.895 kg of oxygen in and 1.085 kg of carbon dioxide out a day, on 12.78 MJ of food. (The
2018 edition gave 0.816 and 1.04; the game's 2004 figures, 0.835 and 0.998 on 11.82 MJ, scale to
within 1% of the 2022 edition per unit of food energy.)"

#### A2

**Outside air. CONFIRMED.** NOAA GML, https://gml.noaa.gov/ccgg/trends/ (updated 2026-10-09):
"September 2026: 426.07 ppm" (a year earlier, 424.37 ppm).

#### A3

**How much CO2 is too much. CONFIRMED.** NIOSH Pocket Guide, "Carbon dioxide",
https://www.cdc.gov/niosh/npg/npgd0103.html (last reviewed 2019-10-30): both quotations exact;
symptoms include "headache, dizziness, restlessness, paresthesia; dyspnea (breathing difficulty);".

#### A4

**Plants take up CO2 while lit. UNCONFIRMED.** Not re-read: it points at the row-by-row NASA citations
in `data/life_support.ron`, which are out of this pass's scope. Re-read them before `air_plants`
ships.

#### C1

**Washing hands. CONFIRMED.** CDC, "About Handwashing", https://www.cdc.gov/clean-hands/about/index.html
(dated 2024-02-16): the five steps; "Scrub your hands for at least 20 seconds."; "If soap and water
are not readily available, you can use an alcohol-based hand sanitizer that contains at least 60%
alcohol."; and the "best way" sentence, all exact.

#### C2

**Sanitiser and dirty hands. CONFIRMED** (from CDC's own page; 3.6 can close it). CDC, "About Hand
Sanitizer", https://www.cdc.gov/clean-hands/about/hand-sanitizer.html (dated 2024-03-12): "Hand
sanitizers may not be as effective when hands are visibly dirty or greasy."; "Hand sanitizers might
not remove harmful chemicals, such as pesticides, and heavy metals like lead."; "Sanitizers do not
get rid of all types of germs."

Wording for the design: "Hand sanitiser does not work as well on visibly dirty or greasy hands, does
not remove chemicals such as pesticides or lead, and does not kill every germ: soap and water first."

#### C3

**Urine on crops. UNCONFIRMED.** WHO's 2006 guidelines PDF
(https://iris.who.int/bitstream/handle/10665/78265/9241546859_eng.pdf) returned a 755-byte HTML page,
not the document. Read it by hand before `clean_compost` step 3 ships.

#### M1

**Heavy bleeding. CORRECTED.**
- MedlinePlus, "Bleeding", https://medlineplus.gov/ency/article/000045.htm (review date 2025-10-14).
  The three quotations are exact. Its tourniquet advice, described because the page forbids copying:
  only when continuous pressure has not stopped life-threatening bleeding, on the limb 2 to 3 inches
  above the wound, avoiding the joint (above the joint, toward the body, if need be), not on bare skin
  (pad it or put it over clothing), with the time noted and told to the responders. The page also
  says not to peek at the wound, and to seek medical help right away if a tourniquet was needed.
- DHS, "Stop the Bleed: Applying a Tourniquet" poster,
  https://www.dhs.gov/sites/default/files/publications/STB_Applying_Tourniquet_08-06-2018_0.pdf (file
  dated 2018-08-06; its page last updated 2025-04-10): "A wound on the leg or arm that won't stop
  bleeding is a good candidate for a tourniquet."; "place a tourniquet at least 2-3 inches from the
  wound"; "The tourniquet should be placed between the wound and the heart."; "Do not apply a
  tourniquet over a joint, such as an elbow, knee, wrist, or ankle."; "Note the time that the
  tourniquet was applied."

New wording: "Press directly on the wound with a sterile bandage, clean cloth or even clothing until
it stops; raise the part when you can; if a dressing soaks through, add another on top and keep
pressing. If pressure does not stop life-threatening bleeding from an arm or leg, a tourniquet goes
2 to 3 inches above the wound, between the wound and the heart, never over a joint, over clothing or
padding, with the time noted for the responders. Get emergency help for severe bleeding, and always
after a tourniquet."

#### M2

**Burns. CORRECTED.** The quotations from MedlinePlus are exact; the other bodies D7 asked for were
found, and they agree on a single time a game can teach.
- MedlinePlus, "Burns", https://medlineplus.gov/ency/article/000030.htm (review date 2026-04-09): cool
  water or a cool bath, "not ice water", for "at least 5 to 30 minutes"; and, for a severe burn, do
  not place it in cold water because it can cause shock.
- American Red Cross Scientific Advisory Council, "Burn Cooling" advisory (approved June 2019),
  https://www.redcross.org/content/dam/redcross/docs/Burn%20Cooling%E2%80%94ADVISORY.pdf: cool
  running water "for a minimum of 10 minutes, ideally 20 minutes"; avoid cooling beyond 40 minutes
  because of hypothermia; watch large burns and small children for it; no ice or ice packs; remove
  clothing and jewellery over the burn. It explains that the 2015 AHA and Red Cross guidelines set a
  10-minute minimum and that burn specialists argue for 20.
- NHS, "Burns and scalds", https://www.nhs.uk/conditions/burns-and-scalds/ (last reviewed
  2026-03-31): "Hold the burn or scald under cool running water for 20 minutes."
- The AHA and Red Cross 2024 first aid guidelines' Highlights (read; the full guideline was not)
  say only that the recommendations for initial thermal burns are substantially unchanged since 2015.

New wording: "Cool a burn under cool running water for 20 minutes (MedlinePlus says at least 5 to 30,
the Red Cross at least 10 and ideally 20, the NHS 20), never ice or ice water, and not past 40
minutes; watch a large burn or a small child for getting too cold, and do not soak a severe burn in
cold water. Take off jewellery and clothing near the burn unless it is stuck. Never butter, oil, ice
or household remedies; do not break blisters. Call the emergency number for a burn the size of a palm
or larger, a full-thickness burn, a chemical or electrical burn, signs of shock, smoke breathed in,
or when unsure."

#### R1

**Sleep. CONFIRMED.** CDC, "About Sleep", https://www.cdc.gov/sleep/about/index.html (dated
2024-05-15): adults 18 to 60 "7 or more hours", 61 to 64 "7–9 hours", 65 and older "7–8 hours".

## Where the sources disagree (the design's 3.4, updated)

- **D1. Chlorine dioxide and Cryptosporidium.** Three CDC pages: "will kill" if instructions are
  followed (2024), "somewhat effective" (2025), kills at practical doses and contact times of several
  hours, with few data across water conditions (Yellow Book, 2025). Judgement: the backcountry page
  is the outlier in wording, but the Yellow Book's data caveat keeps the game's "partly" honest.
- **D2. Bleach strength and dose. Reconciled.** CDC's table: 8 drops = "½ mL" = "a little less than ⅛
  teaspoon" per gallon. EPA: 16 drops = 1/4 teaspoon. Ready.gov's 1/8 teaspoon (about 0.62 mL, our
  arithmetic) is about a quarter more than CDC's ½ mL, the same dose band. CDC and EPA double it for
  cloudy or very cold water; Ready.gov and EPA check it by smell.
- **D3. Altitude.** CDC 6,500 ft; EPA "5,000 feet (1,000 meters)", internally inconsistent; the how-to
  uses 1,000 m, the most cautious.
- **D4. Daily water.** Unchanged. The National Academies' figures are adequate intakes, with no upper
  limit set.
- **D5. Sand filters and viruses. Resolved.** Laboratory 4.7 log, typical household use 1.7 log (5
  estimates); CDC 2010 "unknown". The game's "partly" is right.
- **D6. Heat stroke's temperature.** CDC's sheet: 103 °F (39.4 °C) or higher as a sign. MedlinePlus:
  above 104 °F (40 °C). NIOSH (https://www.cdc.gov/niosh/heat-stress/about/illnesses.html, dated
  2026-03-03): the body temperature "can rise to 106°F or higher within 10 to 15 minutes". The game's
  40 °C matches MedlinePlus.
- **D7. Cooling a burn. Resolved** (M2): 20 minutes fits all three. They differ on severe burns:
  MedlinePlus says not to put a severe burn in cold water; the Red Cross cools all thermal burns and
  watches for hypothermia.
- **D8. Jerky temperatures. Resolved** (F8). `recipes.csv` was corrected in BUG-216.
- **D9. What a person breathes. Resolved** (A1): three editions; the 2022 one is current; the game's
  2004 ratios agree with it per unit of food energy.
- **D10. (New) A drink in a heat emergency.** CDC's sheet: nothing to drink in heat stroke; sip water
  in heat exhaustion. MedlinePlus: sips for anyone alert, nothing by mouth if vomiting or
  unconscious. The game follows CDC for heat stroke and adds MedlinePlus's rule (H8).
- **D11. (New) When a soaked dressing means a tourniquet.** MedlinePlus: add another dressing and keep
  pressing; a tourniquet only if continuous pressure fails on life-threatening bleeding. DHS Stop the
  Bleed: if pressure does not stop it "and the dressing becomes soaked with blood, you will need to
  apply a tourniquet". Same order, a different trigger. The full 2024 AHA and Red Cross guideline,
  which would settle it, was not readable.
- **D12. (New) Wild carrot's purple floret.** Missouri's hemlock page: "has"; its wild carrot page:
  "usually". The game teaches "usually" and never the floret alone.
- **D13. (New) Distillation and chemicals.** Ready.gov: removes "most other chemicals". Nebraska
  Extension and Connecticut DPH: not volatile ones. The two specific sources win over the general one
  (judgement, but the physics is not in doubt: a chemical that boils with the water condenses with
  it).

## What is still unknown, and what would settle it

- **A solar still's daily yield (W22).** Settled by Jackson and van Bavel, Science 1965 (science.org
  refuses automated reading: read it by hand) or the US Army survival manual ATP 3-50.21 (would not
  load), or the cited sources of the 2025 medRxiv preprint. Cost: an hour by hand.
- **The WHO 2006 excreta guidelines (C3).** Read by hand from WHO's archive. An hour.
- **The crop gas-exchange citations (A4).** Re-read the NASA sources cited in `data/life_support.ron`.
- **The full 2024 AHA and Red Cross Guidelines for First Aid** (Circulation, doi
  10.1161/CIR.0000000000001281; ahajournals.org refused): would settle D11 and confirm the burn
  cooling minimum.
- **A number for chlorine dioxide's wait.** The Yellow Book says "several hours"; a product label or
  its EPA registration would give a figure for the recipe.
- **Water intoxication.** The National Academies set no upper limit; a clinical source (MedlinePlus
  on hyponatremia) would give the warning signs for the hydration chain's notice.
- Unchanged from the design's 3.6, not researched in this pass: clay body and firing for a pot filter;
  how long a sand filter's biological layer takes to mature; what home water tests detect (CDC's
  rainwater page says to ask the local health department what to test for); salt curing (FSIS);
  general drying times (NCHFP); a home CO2 comfort target; air-handler condensate as drinking water.

## Sources that could not be reached

| URL | Result | Hoped for |
|---|---|---|
| https://www.redcross.org/take-a-class/resources/learn-first-aid/burns | HTTP 403 | Red Cross burn steps (the 2019 advisory PDF loaded instead) |
| https://redcross.org.au/firstaid/basics/burn | HTTP 403 | a second 20-minute body |
| https://www.ahajournals.org/doi/10.1161/CIR.0000000000001281 | HTTP 403 | the 2024 first aid guideline: burns, tourniquets, heat stroke |
| https://cpr.heart.org/-/media/CPR-Files/CPR-Guidelines-Files/2024-Highlights/Highlights-2024-FA-240927.pdf | HTTP 403 to `curl` | the Highlights (read instead from the u1-cd-cpr.sc.heart.org copy of the same file) |
| https://www.stopthebleed.org/ | loads, steps only in a video | Stop the Bleed steps (the DHS poster was read instead) |
| https://iris.who.int/bitstream/handle/10665/78265/9241546859_eng.pdf | 755-byte HTML stub | WHO 2006 excreta guidelines, volume 4 (C3) |
| https://www.cdc.gov/mmwr/volumes/75/wr/mm7520a2.htm, the CDC vitamin C PDF | HTTP 403 to `curl` | byte-exact text; both were read through the web reader instead |
| FSIS, FoodSafety.gov, science.org | not attempted (known to refuse automated reading) | FSIS's jerky page and chart; Jackson and van Bavel 1965 |

## What it means for this project

**Certain** (what the sources say):

- A pot still or any distiller in the game must not clear solvents or other volatile organic
  compounds (W21's sources), and water with fuel in it is not made safe by boiling (CDC, W6).
- Bleach needs twice the dose for cloudy, murky, coloured or very cold water (CDC, EPA).
- A household sand filter and a ceramic pot filter do not make water safe from viruses in typical use
  (1.7 and 1.3 to 1.9 log).
- Iodine is not for pregnant people, people with thyroid problems or an iodine sensitivity, and not
  for more than a few weeks (CDC).
- The methods whose sources agree they handle every germ class are boiling, distillation, UV in
  clear water, reverse osmosis and ultrafilters of about 0.01 micron (chlorine dioxide is claimed by
  one CDC page and doubted by another, D1); of those, only distillation and reverse osmosis also take
  out salt.
- Burn cooling: 20 minutes under cool running water satisfies MedlinePlus, the Red Cross and the NHS.
- Poultry jerky: 165 °F, from USDA through the University of Kentucky.
- Heat stroke: CDC's sheet gives nothing to drink; MedlinePlus gives nothing by mouth to anyone
  vomiting or unconscious.
- NASA's current life support baseline is the February 2022 edition (REV2).

**Judgement calls** (this pass's opinion; a reader may weigh them differently):

- Keep chlorine dioxide at "partly" for Cryptosporidium, and only after a full wait, because CDC's own
  pages differ and the Yellow Book says data across water conditions are few.
- Keep the game's air loop on the 2004 ratios (they match the 2022 edition per unit of food within
  1%), and show the 2022 numbers wherever a player reads a figure.
- Teach 20 minutes for a burn: one number a person can remember, inside every source's range.
- The `food_forage` identification step should teach a player to leave both plants alone, not to pick
  wild carrot: Missouri's advice for anyone inexperienced is to treat the whole family as deadly, and
  a smell test means crushing a plant whose sap blisters.
- Show the heat warning before 40 °C: CDC lists 103 °F (39.4 °C) as a sign, and the game's
  `heat_stroke` status at 40 °C matches MedlinePlus but leaves no warning in between.

**Knock-on edits outside section 3 of the design** (for the designer; not made in this pass, which
edits section 3 only):

1. `water_store`'s note says CDC's store is "for drinking and sanitation": CDC says drinking, cooking
   and brushing teeth (W26).
2. `water_disinfect` step 3 ("2 drops, wait 30 minutes") is for clear water; the how-to and the recipe
   need the cloudy-water double dose (W9).
3. `water_distil` and P5: distillation clears germs, salt and metals, never fuel or solvents (W21).
4. `water_filter_ceramic`: the recipe must not clear viruses (1.3 to 1.9 log); how much of the
   bacteria and parasites it clears is a game choice from the review's Table 1, labelled so (W14).
5. `aid_burn` step 2 should name the 20 minutes (M2).
6. `warm_alarm` step 3 counts floors only; USFA also asks for one in every bedroom and outside each
   sleeping area (H4).
7. `food_forage` step 3 as above (F15).
8. `water_reclaim` and `air_first` text that quotes NASA figures: use the 2022 edition (A1, W23).

> **Not a doctor, nurse, food scientist or lawyer.** This file records what the sources above said on
> 2026-10-30. It is not medical advice. In an emergency, call the emergency number for where you are.
