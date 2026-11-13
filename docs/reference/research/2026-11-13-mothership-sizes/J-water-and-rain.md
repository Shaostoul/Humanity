# Helper notes J: Hydrogen and water, plant water use, rain in a spinning drum (README section 11)

Part of the [mothership sizes research](README.md) (2026-11-13). A research helper wrote these
notes on 2026-11-12 from the sources it could read; the README carries the answers and the
judgements. After the helper finished, every cited host's robots.txt was checked: findings
read from a page that refuses Claude's readers are marked REMOVED and their numbers are not used.
Quotations were spot-checked against their sources where the README says so; the rest are the
helper's readings, with the helper's own confidence. The helper's own robots notes and summary
table were written before that check: where they name a REMOVED finding or say a host allowed
reading, the check below the README's "How this was read" section is the one that holds.


Context: 250 m radius, 2 km long drum, 1 g at the floor, 1.89 rpm, spine 30 m radius. The README does the trajectory arithmetic; this file supplies sourced inputs.

NOTE ON LIMITS: the web-search tool ran out of budget at the start of this job (every WebSearch call was refused), so every page below was reached by a guessed or known URL. That is why several questions (Gunn and Kinzer fall speeds, fog droplet sizes, drip/sprinkler efficiency, WHO minimum water) have gaps. They are listed honestly under "Negative findings and gaps".

## Robots checked

| Host | Result |
|---|---|
| www.energy.gov | robots.txt read. No rule for ClaudeBot, Claude-User or anthropic-ai; `*` group blocks only admin/search/login paths. Pages used are allowed. |
| www.nasa.gov | read. `User-agent: *  Allow: /`. Allowed. |
| ntrs.nasa.gov | read. `User-agent: *  Allow: /`. Allowed. |
| www.fao.org | read. `*` blocks only /index.php, /typo3/ and a few upload folders. Allowed. |
| www.who.int | read. A bad-bot blocklist naming other bots; none of ours and no `*` block. Allowed (the pages I tried did not hold the figures). |
| extension.umn.edu | read (no page used in the end). Allowed apart from query-string URLs. |
| extension.colostate.edu | read. Only /wp-admin and search blocked. Page guessed returned 404. |
| extension.psu.edu | read. Crawl delay 10 s; blocks catalog/search paths. Page guessed returned 404. |
| www.noaa.gov | read. `*` blocks admin/search/oembed paths only. Allowed. |
| forecast.weather.gov | robots.txt returned 404 (no robots file, so nothing disallows). NWS glossary pages used. |
| webbook.nist.gov | read. Crawl delay 5 s, `ai-train=no` (training disallowed; reading for a task is `ai-input=yes`, allowed). Two molecular-weight pages used, once each. |
| www.weather.gov | robots.txt 404 (no file). Only a redirect was seen. |
| journals.ametsoc.org | robots.txt returned 403. Not read (Gunn and Kinzer 1949 lives here). |
| iris.who.int | robots.txt returned 403. Not read. |
| settlement.arc.nasa.gov | host did not resolve. |
| ww2010.atmos.uiuc.edu | connection refused. |
| staff.science.uva.nl | robots.txt and the PDF both 404. |
| en.wikipedia.org, www.britannica.com | used ONLY to find primary sources / check access; not cited as sources. |

---

## 1. Hydrogen, water and the oxygen loop

### J1. A hydrogen fuel cell makes only electricity, water and heat
- The number(s): water is the only chemical product (apart from electricity and heat).
- Source: Fuel Cells, US DOE Hydrogen and Fuel Cell Technologies Office, https://www.energy.gov/eere/fuelcells/fuel-cells, source's own date: August 14, 2013 (last updated October 9, 2026)
- Quotation: "If hydrogen is the fuel, the only products are electricity, water, and heat."
- Read: 2026-11-12
- Confidence: high, US government agency page.
- Use for the ship: confirms burning hydrogen yields water, but see J4: that water was the water you split to get the hydrogen.

### J2. Electrolysis: how much electricity per kg of hydrogen (today and DOE target)
- The number(s): system energy use 55 kWh per kg H2 (2022 status, 61% of the fuel's lower heating value); DOE 2026 target 51 kWh/kg (65%). The stack alone: 51 kWh/kg status, 48 kWh/kg target. About 6.2 kWh per kg of water split at 55 kWh/kg H2 (55 / 8.94, my arithmetic from J3).
- Source: DOE Technical Targets for Hydrogen Production from Electrolysis, US DOE Hydrogen and Fuel Cell Technologies Office, https://www.energy.gov/eere/fuelcells/doe-technical-targets-hydrogen-production-electrolysis, last updated May 29, 2026
- Quotation: (table values, no sentence) "55 kWh/kg H2 (61% LHV)" system 2022 status; "51 kWh/kg H2 (65% LHV)" 2026 target. These strings were relayed by the fetch tool's reading of the table; treat as paraphrase of a table.
- Read: 2026-11-12
- Confidence: medium-high (agency table, current; the table was read through the fetch tool's summary).
- Use for the ship: plan 50 to 55 kWh per kg of hydrogen made; about 6 kWh per litre of water split.
- Supporting: the DOE electrolysis explainer, https://www.energy.gov/eere/fuelcells/hydrogen-production-electrolysis (updated October 13, 2026): "Electrolysis is the process of using electricity to split water into hydrogen and oxygen" (first 88 characters of the sentence; the tool truncated the rest).

### J3. Stoichiometry: about 8.94 kg of water per kg of hydrogen burned
- The number(s): 18.0153 / 2.01588 = 8.937 kg of water per kg of H2 (2 H2 + O2 gives 2 H2O; one H2O is 18.0153 g/mol and one H2 is 2.01588 g/mol). Oxygen consumed: 7.94 kg per kg H2.
- Source: NIST Chemistry WebBook, https://webbook.nist.gov/cgi/cbook.cgi?ID=C7732185 (water) and ...?ID=C1333740 (hydrogen), undated pages
- Quotation: "Molecular weight: 18.0153" (water); "2.01588" (hydrogen, H2). The division is my arithmetic.
- Read: 2026-11-12
- Confidence: high (NIST reference data); the ratio is simple arithmetic.
- Use for the ship: 9 kg water per kg H2 is right to 1%.

### J4. The ISS makes oxygen by splitting recovered water, and the hydrogen is reused or vented
- The number(s): no figure; process description.
- Source: Environmental Control and Life Support Systems (ECLSS), NASA, https://www.nasa.gov/reference/environmental-control-and-life-support-systems-eclss/, source's own date: updated Apr 4, 2025
- Quotation: "the cell stack, which electrolyzes, or breaks apart, water provided by the Water Recovery System" ; "yielding oxygen and hydrogen as byproducts." ; "the hydrogen is either vented into space or fed to the carbon dioxide reduction assembly."
- Read: 2026-11-12
- Confidence: high, NASA reference page (quotes as returned by the fetch tool, which spliced fragments; the first and third are verbatim phrases).
- Use for the ship: the ISS oxygen supply is itself a water loop: water in, oxygen to the cabin, hydrogen to a Sabatier reactor with the crew's CO2. This supports the README's view that hydrogen is not a water SOURCE.

### J5. ISS water recovery: 98%
- The number(s): total water recovery 98% (was 93 to 94% before the brine processor).
- Source: NASA, "NASA Achieves Water Recovery Milestone on Space Station", https://www.nasa.gov/humans-in-space/nasa-achieves-water-recovery-milestone-on-international-space-station/, source's own date: Jun 20, 2023 (last updated Mar 02, 2026)
- Quotation: "We have now demonstrated that we can reach total water recovery of 98%, thanks to the brine processor."  Also: "Before the BPA, our total water recovery was between 93 and 94% overall,"
- Read: 2026-11-12
- Confidence: high, NASA's own announcement. Note the page does not itself say where ISS oxygen comes from; J4 does.
- Use for the ship: a loop that loses 2% per pass needs makeup of 2% of the water throughput; for a drum, hydrogen from outside or from asteroid ice is the makeup, not a daily supply.

---

## 2. How much water plants and people need

### J6. Reference evapotranspiration (ETo): FAO 56 Table 2
- The number(s): average ETo in mm/day (1 mm over 1 m2 = 1 kg of water, so 1 mm/day = 10 m3 per hectare per day):
  - Tropics/subtropics, humid: cool 2 to 3, moderate (20 C) 3 to 5, warm (over 30 C) 5 to 7
  - Tropics/subtropics, arid: 2 to 4, 4 to 6, 6 to 8
  - Temperate, humid: 1 to 2, 2 to 4, 4 to 7
  - Temperate, arid: 1 to 3, 4 to 7, 6 to 9
- Source: Crop evapotranspiration: Guidelines for computing crop water requirements (FAO Irrigation and Drainage Paper 56), Allen, Pereira, Raes, Smith, FAO, http://www.fao.org/4/x0490e/x0490e04.htm (Table 2), published 1998
- Quotation: Table 2 title "Average ETo for different agroclimatic regions in mm/day" (the table values are as listed above; the page adds that they "are not intended for direct application").
- Read: 2026-11-12
- Confidence: high (the FAO standard method).
- Use for the ship: a 20 C temperate garden, humid air: 2 to 4 mm/day; take 4 mm/day (40 m3/ha/day) for sunlit vegetables, then scale by crop coefficient (J7) and by the lamp or sunlight actually supplied.

### J7. Crop coefficients (Kc): crop ET = Kc x ETo
- The number(s): Kc mid-season: lettuce 1.00, tomato 1.15 (up to 1.20 when staked tall), Solanaceae vegetables 1.15 (Kc ini 0.6, Kc end 0.80). So tomato at ETo 4 mm/day needs about 4.6 mm/day.
- Source: FAO Irrigation and Drainage Paper 56, chapter 6, Table 12, https://www.fao.org/4/x0490e/x0490e0b.htm, 1998
- Quotation: Table 12 caption "Single (time-averaged) crop coefficients, Kc, and mean maximum plant heights for non stressed, well-managed crops" ; row "Lettuce / 1.00 / 0.95 / 0.3" (as returned by the fetch tool).
- Read: 2026-11-12
- Confidence: high for open field; FAO says these are for standard (non-greenhouse) conditions.
- Use for the ship: use 1.0 to 1.2 on the local ETo.

### J8. NASA plant water use per square metre per day
- The number(s): water uptake / transpiration for crops in NASA's table, nominal values 1.24 to 4.70 kg/m2/day (spinach 1.24; most grain and vegetable rows 1.74 to 2.88; one row 4.70; the table's final row 4.00); a typical middle is about 2 to 3 kg/m2/day = 2 to 3 mm/day = 20 to 30 m3/ha/day. Separately, the BVAD Table 4-20 gives "Biomass production water consumption" of 4.00 kg per m2 per day for a mature planetary base (0.10 for the Devon Island analog).
- Source: Life Support Baseline Values and Assumptions Document (BVAD), NASA/TP-2015-218570 Rev 2, Ewert, Chen, Powell, NASA JSC, https://ntrs.nasa.gov/citations/20210024855, source's own date: February 2022. Tables 4-91 and 4-20.
- Quotation: Table 4-91 column heading "Average Water Uptake / Transpiration" with unit "[kg/m2d]"; Table 4-20 row "Biomass Production Water Consumption" with "4.00" in the Mature Planetary column. (Read from the PDF text through pdftotext; the column alignment in Table 4-91 was garbled, so the per-crop labelling is not certain; the range is.)
- Read: 2026-11-12
- Confidence: medium (primary NASA table, but I could not read crop names against numbers cleanly).
- Use for the ship: design for 3 mm/day average, 5 mm/day peak, consistent with J6/J7.

### J9. Water a person drinks and uses (NASA)
- The number(s): ISS drinking water 2.00 kg/person/day plus food rehydration 0.50, total human consumption 2.50 kg/person/day. Hygiene total on ISS: 0.7 kg; mature planetary base hygiene (shower 1.08, laundry 1.1, dish wash 3.54, etc.) 6.62 kg; total water consumption mature planetary base 9.12 kg/person/day. Crew breath and sweat put 1.42 kg/person/day (range 0.8 to 2.45) of water into the air.
- Source: BVAD NASA/TP-2015-218570 Rev 2 (as J8), Table 4-20 "Typical Steady-State Water Usage Rates for Various Missions" and a note on respiration and perspiration, February 2022
- Quotation: table title "Typical Steady-State Water Usage Rates for Various Missions"; "1.42 kg/CM-d with a range of 0.8 to 2.45 kg/CM-d" (this second phrase is verbatim from the PDF text).
- Read: 2026-11-12
- Confidence: high for crew; medium for how the table columns line up.
- Use for the ship: about 10 L per person per day for a spacecraft-style closed-loop use (people), versus the roughly 250 to 400 L a person uses at home. For 500 people per sector: about 5 m3/day for personal use on the NASA planetary-base figure.

### J10. Space Settlements study (SP-413): water in a colony
- The number(s): total water use per person is 75 percent of US domestic use, with about 250 kg per person per day of recycled water just for waste transport; "25 times the potable water needed" circulates; 200 kg per person is held in reserve for emergencies and fire; plant mass balance (Table 5-19) lists irrigation water 168,750 g and evapotranspiration 164,900 g per person-day with the footnote "Based on: 30.5 cm water on 45 m2 in 60-day season" (30.5 cm over 60 days is 5.1 mm/day). 61 ha of farmland feeds 10,000 people in its design.
- Source: Space Settlements: A Design Study, NASA SP-413, 1977, https://ntrs.nasa.gov/citations/19770014162 (PDF https://ntrs.nasa.gov/api/citations/19770014162/downloads/19770014162_update.pdf), chapter 5 and Table 5-19, published 1977
- Quotation: "Based on: 30.5 cm water on 45 m2 in 60-day season" (the text was OCR'd and digits spaced out in the PDF, so I give it normalised); "provides 25 times the potable water needed to satisfy" (OCR).
- Read: 2026-11-12
- Confidence: medium (primary source but OCR text and the units of the 168,750 g are my reading of "per person per day").
- Use for the ship: confirms about 5 mm/day irrigation for a 1970s design crop mix, as a sanity bound against J6/J8.

### J11. Condensing it all back out (dehumidifier load)
- SP-413 text: "Humidity control is achieved by cooling the air below the dewpoint, condensing the moisture and separating it." and (water supply) "Water condensed from the air is heated to 16 C and fed into the fish ponds." Also humidity target "Relative humidity = 50 +/- 10 percent".
- Source: SP-413 (as J10), same PDF, 1977. Quotations OCR-spaced; verbatim wording normalised.
- Read: 2026-11-12
- Confidence: medium.
- Use for the ship: every litre transpired must be condensed; the heat to remove is the latent heat of vaporization (about 2.45 MJ per kg, about 0.68 kWh per kg: a standard physical constant, NOT sourced in this run). At 3 mm/day per hectare = 30 m3/day = 20 MWh of cooling per day per hectare, which equals the sunlight's heat that drove it, so the condenser is also the ship's heat sink. Flag for the README: that cooling has to reach the radiators.

---

## 3. Raindrops, drizzle, fog

### J12. Drizzle and rain: size definitions (NWS)
- The number(s): drizzle is drops under 0.5 mm; rain is drops over 0.5 mm.
- Source: NWS Glossary, "drizzle" and "rain", NOAA National Weather Service, https://forecast.weather.gov/glossary.php?word=drizzle and ?word=rain, undated
- Quotation: "Precipitation consisting of numerous minute droplets of water less than 0.5 mm (500 micrometers) in diameter." ; "Precipitation that falls to earth in drops more than 0.5 mm in diameter."
- Read: 2026-11-12
- Confidence: high.
- Use for the ship: drizzle is under 0.5 mm, rain 0.5 mm and up.

### J13. Fog: NWS definition only
- Source: NWS Glossary "fog", https://forecast.weather.gov/glossary.php?word=fog, undated
- Quotation: "Fog is water droplets suspended in the air at the Earth's surface."
- Read: 2026-11-12
- Confidence: high for the definition; it gives no droplet size.
- Use for the ship: no droplet size sourced; see gaps.

### J14. Raindrop fall speed vs size (Gunn and Kinzer): NOT obtained from a primary source
- What I could get: a SECONDARY page (Wikipedia, not a source for us) states "A droplet with a diameter of 3 mm has a terminal velocity of approximately 8 m/s" (citing a university paper that returned 404) and, for rain, 0.5 mm drops at about 2 m/s and 5 mm drops at about 9 m/s. From memory (NOT sourced, do not cite): Gunn and Kinzer 1949 give about 4 m/s at 1 mm, 6.5 at 2 mm, 8 at 3 mm, 9 m/s from 4 to 5.5 mm; drops above about 5 to 6 mm break up.
- Source: none primary. journals.ametsoc.org returned 403 on robots.txt, so I did not read it. The NOAA JetStream pages (https://www.noaa.gov/jetstream/atmosphere/precipitation, updated May 20, 2024) were read and carry no sizes or speeds.
- Confidence: low.
- Use for the ship: use 2 mm drops falling at about 6.5 m/s and 4 mm at about 9 m/s only as working numbers until a primary table is read; note that in a 1 atm drum the fall is mostly terminal speed after about 1 to 2 s.

### J15. Greenhouse fog droplet size: NOT found
- Two extension pages guessed (extension.psu.edu, extension.colostate.edu) returned 404; no figure read. See gaps.

---

## 4. Falling things in a rotating habitat

### J16. SP-413: a jump lands off to the side
- The number(s): jump height 0.55 m in the 1 rpm class torus (the study's reference habitat): lands about 5.3 cm to the side.
- Source: Space Settlements: A Design Study, NASA SP-413, https://ntrs.nasa.gov/citations/19770014162, chapter 3 ("Figure 3-1" text), 1977
- Quotation: "he would not come straight down, but would land about 5.3 cm (more than 2 in.) to one side." (text OCR; spaces normalised)
- Also: "1 rpm is set as the upper limit of permissible rotation rate for the principal living quarters" and "people can eventually adapt to rates below 3 rpm".
- Read: 2026-11-12
- Confidence: high that the passage exists; the reference radius is not stated in the passage I read (the study's torus was 830 to 900 m; check before comparing to the drum). 
- Use for the ship: this is a short-fall example: a 0.55 m jump takes about 0.67 s up and down; the README can scale to the drum (1.89 rpm is well above the study's 1 rpm limit, so Coriolis effects are about twice as big at the same speeds; the study says rates above 1 rpm should be avoided for general population).

### J17. Hall's artificial gravity papers and NASA technical reports on rain
- Not read. NTRS keyword search for "raindrop terminal fall velocity" returned only disdrometer papers; no treatment of rain in a rotating colony was found. SP-413 has no rain treatment (its text lines on rain talk only of Earth's weather and "Hail, rain, wind: None" in a table of protection).
- Use for the ship: the rain-in-a-spin trajectory is the README's arithmetic; there is no published solution to quote.

---

## 5. Irrigation methods and efficiency

### J18. FAO: sprinkler and drip are more efficient than surface irrigation; wind spoils sprinklers
- The number(s): no percentages (see gaps).
- Source: Irrigation Water Management: Irrigation Methods, FAO Training Manual 5, Chapter 7, http://www.fao.org/4/s8684e/s8684e08.htm, undated
- Quotation: "is generally higher with sprinkler and drip irrigation than surface irrigation" ; "efficiency is just as much a function of the irrigator as the method used." ; "Strong wind can disturb the spraying of water from sprinklers"
- Read: 2026-11-12
- Confidence: medium (as returned by the fetch tool).
- Use for the ship: The 90% drip and 75% sprinkler figures are NOT confirmed by a source I could read. In a closed ship losses are not lost (they drain, condense, and come back), so efficiency matters for pumping and condensing energy, not for water balance.

---

## Could not read
- https://journals.ametsoc.org/robots.txt, 403 (Gunn and Kinzer 1949 not read)
- https://iris.who.int/robots.txt and the WHO WSH03.02 PDF, 403 (WHO minimum domestic water not obtained)
- https://www.who.int/publications/i/item/9789240015241, page read but holds no litres-per-day figure
- https://www.who.int/water_sanitation_health/diseases/WSH03.02.pdf, 404
- https://settlement.arc.nasa.gov, host not found
- http://staff.science.uva.nl/~jboxel/Publications/PDFs/Gent_98.pdf, 404
- http://www.fao.org/4/s8684e/s8684e06.htm, the fetch tool returned an error, not retried
- https://www.fao.org/4/x0490e/x0490e0b.htm, no greenhouse ET text (page covers open-field standard conditions)
- DOE electrolysis page https://www.energy.gov/eere/fuelcells/hydrogen-production-electrolysis has no kWh/kg (got them from the targets page, J2)
- https://extension.colostate.edu/.../sprinkler-irrigation-4-715/ and https://extension.psu.edu/greenhouse-cooling-and-humidification-with-fog-systems, 404 (guessed URLs)
- NWS JetStream drop-size pages, 404

## Negative findings and gaps
- Web search was disabled during this run; there is no sourced Gunn and Kinzer table, no extension source for fog droplet size (microns) or for fog evaporating before landing, no extension/FAO source for drip about 90% and sprinkler about 75%, no peer-reviewed greenhouse transpiration per m2 beyond NASA's table, and no WHO minimum-water figure. Follow-up: allow searches, or the README supplies URLs. Candidate primary sources to read: Gunn and Kinzer 1949 (Journal of Meteorology 6, 243), Howard and Bartram 2003 WHO "Domestic water quantity, service level and health", FAO Irrigation Water Management manuals, university greenhouse fogging bulletins.
- The DOE electrolysis explainer page gives no kWh/kg; use the DOE technical-targets table.
- NASA's ISS recovery page does not say where ISS oxygen comes from; the ECLSS page does.
- No published treatment of rain falling in a rotating habitat was found (NTRS keyword search, SP-413). SP-413 gives only the jump example.
- FAO 56 says nothing about greenhouse ET; do not apply its Kc to a lit, closed deck without adjusting for the lighting.
- SP-413 latent-heat numbers: not read; the 2.45 MJ/kg figure above is not sourced.

## Summary table
| Question | Recommended number | Findings | Confidence |
|---|---|---|---|
| Fuel cell product | water only (plus electricity and heat) | J1 | high |
| Electrolysis energy | 55 kWh/kg H2 now, 51 kWh/kg DOE 2026 target; about 6.2 kWh per kg of water split | J2, J3 | medium-high |
| Water per kg H2 burned | 8.94 kg | J3 | high |
| ISS oxygen source and recovery | electrolysis of recovered water; 98% total water recovery (2023) | J4, J5 | high |
| Plant water use | 3 mm/day average, 5 mm/day peak (ETo 2 to 7 mm/d times Kc 1.0 to 1.2; NASA 1.2 to 4.7 kg/m2/d) = 30 m3/ha/day average, 50 peak | J6 to J8, J10 | medium-high |
| People | 2.5 kg/day drink and food (ISS), about 9 kg/day all uses planetary base; 1.4 kg/day of breath and sweat humidity | J9, J10 | high |
| Dehumidifier load | all transpired water must be condensed; about 0.68 kWh (unsourced constant) per kg, so about 20 MWh/day per hectare at 3 mm/d | J11 | medium |
| Drop size | drizzle under 0.5 mm, rain over 0.5 mm | J12 | high |
| Drop fall speed | working numbers only: 2 mm about 6.5 m/s, 4 mm about 9 m/s (NOT sourced) | J14 | low |
| Fog droplet size | gap | J15 | none |
| Coriolis on a falling object | SP-413: 0.55 m jump lands 5.3 cm to the side; limit 1 rpm, drum is 1.89 rpm | J16 | high on the quote |
| Sprinkler/drip efficiency | gap (90%/75% unconfirmed); FAO says sprinkler/drip better than surface | J18 | low |

## Recommendation (my judgement, not a sourced finding)
1. Hydrogen burning is not a water source aboard. Every kilogram of H2 burned returns 8.94 kg of water, but you only had that hydrogen because you spent about 6 kWh per kg of water (55 kWh per kg H2) splitting the same water; the ISS does exactly this loop (J2 to J4). Burn hydrogen as an energy store or rocket fuel, and count its water as a return, not a supply. Makeup water is only the loss rate (NASA: 2% per pass on ISS, J5).
2. Water the gardens by booms/drip near the plants, with a mist or light sprinkler only where leaf wetting is wanted; use rain from the spine only as a visual or cooling effect: drops that fall 220 m (spine to floor) arrive after about 30 s at 6 to 9 m/s terminal speed and get pushed sideways by Coriolis, and drizzle/fog under 0.5 mm barely reaches the floor. All of this is the README's arithmetic; I could not source fog size or fall-speed tables.
3. Daily water per hectare of farm: about 30 m3 (3 mm), peak 50 m3 (5 mm), all of it to be condensed back out of the air each day. A 500-person sector's own use is about 5 m3/day (10 L per person), so farms dominate the water cycle by roughly 100 to 1 per hectare.
