# Helper notes C: Housing tiers and the area one person needs to feed themselves (README section 3)

Part of the [mothership sizes research](README.md) (2026-11-13). A research helper wrote these
notes on 2026-11-12 from the sources it could read; the README carries the answers and the
judgements. After the helper finished, every cited host's robots.txt was checked: findings
read from a page that refuses Claude's readers are marked REMOVED and their numbers are not used.
Quotations were spot-checked against their sources where the README says so; the rest are the
helper's readings, with the helper's own confidence. The helper's own robots notes and summary
table were written before that check: where they name a REMOVED finding or say a host allowed
reading, the check below the README's "How this was read" section is the one that holds.


Drum used for the arithmetic: 250 m radius, 2 km long, so the inner floor is 2 x pi x 250 x 2000 = 3,141,593 m2 (3.14 km2). "Drum floor per person" below means the ground the tier occupies (building footprint, plus yard where the tier has one). Dividing 3,141,593 m2 by it gives how many people the whole floor would hold if every square metre were that tier (no roads, farms, machinery, so it is a ceiling, not a plan).

Conversions: 1 ft2 = 0.0929 m2; 1 acre = 4,047 m2; 1 ha = 10,000 m2.

## 1. Barracks and the floor under everyone

### C1. US Navy/Marine Corps open bay: net living area per person
- REMOVED 2026-11-12: nibs-s3-wbdg3-production.s3.us-east-1.amazonaws.com (the UFC 4-721-10 PDF host) disallows Claude-User. Its numbers are not used anywhere in this research.

### C2. The same UFC: room-based unaccompanied housing, per person
- REMOVED 2026-11-12: same host as C1. Its numbers are not used anywhere in this research.

### C3. Army troop-housing floor space for basic training
- The number(s): 72 ft2 = 6.7 m2 per person normal minimum; never below 55 ft2 = 5.1 m2 when 72 cannot be met.
- Source: Environmental Health Inspections and Surveys I (MD0164), Section III "Public Health Aspects of Troop Housing", US Army Medical Department subcourse, armymedical.tpub.com/MD0164/Minimum-Adequacy-Standards-cont-Environmental-Health-Inspections-and-Surveys-I-28.htm, undated
- Quotation: "a minimum of 72 square feet of floor space per person" and "the minimum per individual should not be less than 55 square feet"
- Read: 2026-11-12
- Confidence: medium (a republished Army training text; I did not find the Army regulation itself, AR 420-1 Table 3-10, in readable form)
- Use for the ship: confirms 6.7 m2 and gives a crowding floor of 5.1 m2.

### C4. Army transient-training barracks: open bay maximum
- The number(s): maximum 90 NSF = 8.4 m2 living area per soldier for open bay, central latrine and shower, E1-E6.
- Source: ORTC Army Standard (Army transient training barracks), USACE MRSI library (mrsi.erdc.dren.mil); I saw this only as a search-result summary of the PDF, not the page
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: low
- Use for the ship: consistent with C1 MOS trainee 8.4 m2; do not cite on its own.

### C5. Minimum covered living space (humanitarian floor)
- The number(s): 3.5 m2 per person in warm climates, excluding kitchen; 4.5 to 5.5 m2 per person indoor living space in cold climates (where cooking, bathing, sanitation are inside).
- Source: Emergency Handbook, "Shelter and settlement"-type page, UNHCR, emergency.unhcr.org/node/538, undated (the page says it follows the SPHERE standards, 2018; it does not label these exact figures as Sphere's)
- Quotation: "Minimum 3.5m2 of covered living space per person in tropical or warm climates, excluding cooking facilities or kitchen."  Cold: "Minimum 4.5m2 to 5.5m2 indoor living space per person."
- Read: 2026-11-12
- Confidence: high for UNHCR's figure; medium that it equals the current Sphere text (I could not read Sphere's own site, see Could not read)
- Use for the ship: 4.5 m2 is the hard floor under every tier (the ship is a sealed, climate-controlled interior, so use the indoor figure, not 3.5).

### C6. UK MoD Single Living Accommodation
- The number(s): none found. The 2013 SLA Design Guide (PG0812) says room areas come from the JSP 315 Accommodation Scales (Scale 3 Junior Ranks, Scale 30/35/32 officers/SNCOs/cadets), and says the bedroom and en-suite areas are combined, but gives no m2.
- Source: SLA Design Guide PG0812, UK MoD, assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/172888/20130326_PG0812_SLA_Design_Guide.pdf, dated 26 March 2013 (from the file name)
- Quotation: none (text searched locally for m2 and area; no figures)
- Read: 2026-11-12
- Confidence: n/a (negative finding)
- Use for the ship: skip UK MoD; US figures stand.

## 2. Dormitory

### C7. US new residence halls: gross floor per bed
- The number(s): median 333 ft2 = 31 m2 gross per bed (52 projects, 2014); halls over 500 beds: 303 ft2 = 28 m2 per bed; smaller halls (under 250 beds) almost 366 ft2 = 34 m2. This divides the WHOLE building (corridors, lounges, kitchens, laundry, study rooms) by students housed, so it is not the bedroom size.
- Source: 2014 College Housing Report ("Living on Campus"), College Planning and Management magazine, Paul Abramson, mahlum.com/pdf/CPM2014CollegeHousingReport(1).pdf, 2014
- Quotation: "space allocated per bed in the median project was 333 square feet"
- Read: 2026-11-12
- Confidence: medium (a trade-magazine survey, not a standard; but it is real built data, and it averaged 333 over 12 years)
- Use for the ship: dormitory gross 31 m2 per person including shared kitchens, baths, lounges.

### C8. Dormitory room sizes
- The number(s): no university space standard could be read. A search summary says MIT's 2022 residence standard lists single rooms at 90 to 179 ft2 (8.4 to 16.6 m2) and doubles at 180 to 269 ft2; not verified on the page. The UK floor for any bedroom in a home is in C11 (7.5 m2 single, 11.5 m2 double).
- Source: MIT Design Standards T15 Residences 2022 (snippet only); University of Wisconsin 12A3G residence hall study (PDF over the 10 MB fetch limit, not read); UCSC housing standards (404)
- Quotation: none
- Read: 2026-11-12
- Confidence: low
- Use for the ship: private bedroom 8.4 m2 (single) as the dorm default, matching the Army MOS trainee 8.4 m2.

## 3. Co-housing cluster

### C9. Households per community
- The number(s): "12 to up to 36 households" best (Durrett and McCamant); about 180 US communities "typically with 30 to 40 households each"; UK communities from about 8 to about 30 households (search summary).
- Source: (a) Cohousing guidelines, cnu.org/node/3056, quoting McCamant and Durrett, undated. (b) "Building Community with Cohousing", Rachel MacCleery, Urban Land (Urban Land Institute), Spring 2023, read from cohousing-solutions.com PDF copy
- Quotation: "Anywhere from 12 to up to 36 households seems to work best" and "typically with 30 to 40 households each"
- Read: 2026-11-12
- Confidence: high
- Use for the ship: cluster of 24 to 36 households (about 60 to 90 people).

### C10. Common house and home size
- The number(s): common house about 100 to 150 ft2 per household = 9 to 14 m2; a 2003 tally of completed US communities averaged 3,657 ft2 = 154 ft2 (14.3 m2) per unit; examples 5,400 ft2 for 37 households (146 ft2 = 13.6 m2), 4,500 ft2 for 30 homes. Home sizes: one community average 825 ft2 (77 m2); another range 934 to 1,726 ft2 (87 to 160 m2).
- Source: Cohousing-L mailing list of the Cohousing Association (lists.cohousing.org), posts msg10349, msg17820 and others, via search-result summaries only. The list's own robots.txt fetch returned 403, so I did not read the posts.
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12 (summaries only)
- Confidence: low (practitioner posts, not a standard; consistent with each other)
- Use for the ship: common house 14 m2 per household (about 5.6 m2 per person at 2.5 people). Home sizes: use the UK standard in C11, which I could read.
- Also: Marmalade Lane (Cambridge, 42 homes, 2 to 5 bedroom terraces and flats) lists a 4,300 m2 figure of unclear meaning and site size 0.9 to 0.97 ha (search summary); its Good Homes Alliance case study (published 24 November 2017) lists the common house contents (playroom, guest bedrooms, laundry, hall, kitchen) but no areas. UK Cohousing Network's design guide (47 pages) had no floor figures.

## 4. Row house

### C11. UK Nationally Described Space Standard (gross internal area)
- The number(s): 1b1p 39 m2 (37 with shower room); 2b3p 61 m2 one storey, 70 two storey; 2b4p 70 / 79; 3b5p 86 / 93; 4b6p 99 / 106. Per person: 2b4p two-storey 79/4 = 19.8 m2; 3b5p 93/5 = 18.6; 4b6p 106/6 = 17.7. Built-in storage 1.0, 2.0, 2.0, 2.5, 3.0 m2. Bedrooms: single 7.5 m2 and 2.15 m wide; double or twin 11.5 m2.
- Source: Technical housing standards, nationally described space standard, UK Government (MHCLG), gov.uk/government/publications/technical-housing-standards-nationally-described-space-standard/technical-housing-standards-nationally-described-space-standard, published 27 March 2015 per search listing (not shown in the text I read)
- Quotation: "a single bedroom has a floor area of at least 7.5m2 and is at least 2.15m wide" / "a double (or twin bedroom) has a floor area of at least 11.5m2". The table rows are a table, so no prose quotation for them.
- Read: 2026-11-12
- Confidence: high (national standard; the table values were returned by the page-reading tool and match a second source in search results)
- Use for the ship: row house 79 m2 gross floor for 4 people (19.8 m2 each), 2 storeys, footprint about 40 m2.

### C12. Row house plot width and area per unit (US zoning)
- The number(s): Raleigh NC townhouse minimum lot width 16 ft = 4.9 m (14 ft = 4.3 m in R-10); minimum net site area per unit 7,500 / 4,500 / 3,000 / 2,250 ft2 in R-2 / R-4 / R-6 / R-10 = 697 / 418 / 279 / 209 m2.
- Source: Unified Development Ordinance, townhouse, City of Raleigh, udo.raleighnc.gov/sec-244-townhouse-i, no revision date on the page
- Quotation: "R-10" row: minimum lot width "14'", net site area per unit "2,250 sf" (table cells)
- Read: 2026-11-12
- Confidence: medium (one city; these are low-density zoning ceilings, not the tightest possible)
- Use for the ship: 4.9 m width is a sound row-house frontage. The 209 m2 per unit is the densest US zoning I read; a UK terrace is tighter (next finding).

### C13. Private outdoor space for a terraced or small home (UK)
- The number(s): Essex Design Guide: "outside space of at least 25 sq m" for every home above 50 dwellings per hectare; walled yard around 25 m2; balcony 5 m2 gross where a garden or communal space cannot be provided. Old guidance varied by bedrooms (figures not stated on the page).
- Source: Private Space, Essex Design Guide, essexdesignguide.co.uk/design-details/layout-details/private-space/, undated
- Quotation: "At densities above 50 dwellings per hectar an outside space of at least 25 sq m is expected for all homes."
- Read: 2026-11-12
- Confidence: medium (a county design guide, not national law)
- Use for the ship: row-house yard 25 m2 minimum; 40 m2 recommended (my number, to hold a few raised beds).

## 5. Homestead: land and lit area per person

### C14. Ecology Action / GROW BIOINTENSIVE: complete diet for one person
- The number(s): 4,000 ft2 = 372 m2 for a complete diet at intermediate GROW BIOINTENSIVE yields. A newer Ecology Action newsletter says "as little as 2,000ft2" = 186 m2 "working with good soil and gardening skill". Ecology Action's 60/30/10 plan (60% carbon/calorie crops, 30% income and vegetable crops, 10% compost crops) is the layout.
- Source: (a) FAQ, Raw Food, growbiointensive.org/FAQ/FAQ_RawFood.html, undated. (b) E-newsletter Spring 2024 FAQ, growbiointensive.org/Enewsletter/Spring2024/FAQ.html, Spring 2024 (from its table-of-contents link; no date printed). Vegan complete diet; the FAQ adds a raw-food eater needs more.
- Quotation: (a) "A complete diet for one person realistically takes 4,000 sq ft, when grown at GROW BIOINTENSIVE® intermediate yields." (b) "(as little as 2,000ft2 for a complete diet for one person working with good soil and gardening skill)"
- Read: 2026-11-12
- Confidence: high for what Ecology Action states; it is the group's own claim, not independently measured
- Use for the ship: 372 m2 per person is the soil number for a plant-only complete diet; 186 m2 is an expert best case; treat 372 as the design value.

### C15. Peters et al. 2016: land per person by diet
- The number(s): 0.13 to 1.08 ha per person per year (1,300 to 10,800 m2) across ten US diet scenarios; carrying capacity 402 to 807 million people; the baseline (current) diet needed roughly eight times the land of the vegan diet (Tufts press summary, not the paper). Results hinge on how much cropland counts as cultivable. These figures include whatever pasture and cropland each diet needs, not a garden.
- Source: Carrying capacity of U.S. agricultural land: Ten diet scenarios, Peters et al., Elementa: Science of the Anthropocene 4, 22 July 2016 (abstract read on dspace.allegheny.edu; the journal site returned 403)
- Quotation: "Annual per capita land requirements ranged from 0.13 to 1.08 ha person-1 year-1 across the ten diet scenarios."
- Read: 2026-11-12
- Confidence: high for the abstract; medium for use (field-scale farming, not intensive beds)
- Use for the ship: shows field farming needs 3 to 30 times more land than Ecology Action's beds; the homestead must use intensive methods.

### C16. World average cropland per person
- The number(s): 0.2 ha = 2,000 m2 per person in 2021, down 18 percent from 2000 (FAO analytical brief, via a search summary; the FAO file returned 403). OWID's cropland-per-capita chart exists (HYDE 2025 data, updated 8 June 2026) but the page text did not give the world value.
- Source: FAO analytical brief (openknowledge.fao.org), undated; OWID ourworldindata.org/grapher/cropland-per-capita
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: low-medium (secondary relay of the FAO number)
- Use for the ship: context only: today's world has about 2,000 m2 of cropland per person feeding everyone, including feed for animals.

### C17. The operator's 34 x 34 m (1,156 m2) garden: what published figure matches
- The number(s): 34 x 34 m = 1,156 m2 = 12,443 ft2 = 0.286 acre. A quarter acre is 1,012 m2 (10,890 ft2), about 12 percent smaller, so it does not match exactly. 4,000 ft2 (Ecology Action) is 372 m2, about a third of it. The lowest Peters scenario, 1,300 m2, is the nearest published figure, 12 percent above it.
- Source: arithmetic on C14, C15
- Quotation: none
- Read: 2026-11-12
- Confidence: n/a
- Use for the ship: NEGATIVE FINDING: I found no primary source that says "34 x 34 m" or "1,156 m2 for a year-round garden for one person". A quarter acre is a common rule of thumb but I did not find it in a primary source either. Say plainly the 1,156 m2 is a design choice that is about 3 times Ecology Action's 4,000 ft2 and at the low end of Peters field-farming needs. That is a defensible margin (room for fruit trees, hens, goat yard), not a published requirement.

### C18. NASA BVAD Rev 2 (2022): lit growing area per crew member
- The number(s): Table 4-92 totals (growing area per crew member): salad crops only 1.35 m2; salad plus carbohydrate crops 19.50 m2; all Exploration Life Support crops 65.29 m2. Inedible biomass at those: 0.07, 4.29, 6.66 kg per crew member per day.
- Source: Life Support Baseline Values and Assumptions Document, NASA/TP-2015-218570 Rev 2, Ewert, Chen, Powell, NASA, ntrs.nasa.gov/citations/20210024855 (PDF BVAD_2.15.22-final.pdf), February 2022
- Quotation: the table row prints "Total 1.35 0.07 19.50 4.29 65.29 6.66"
- Read: 2026-11-12
- Confidence: high for the table; the soybean line alone (46 m2) dominates the all-crops total
- Use for the ship: does it supply all calories? NO for the first two: BVAD says the salad diet leaves "the bulk of the nutritional content" to packaged food, and salad plus carbohydrate gives "somewhere around half of the necessary mass". The all-crops diet "provides a high degree of closure" with herbs and condiments still resupplied. So 65 m2 lit area per person is close to a full diet, 19.5 m2 about half.

### C19. NASA review and Wheeler talk: area for a full diet
- The number(s): "about 40 to 50 m2 (or more)" of crop area per person under high light for enough calories; 35 to 40 m2 per person with insects. Wheeler (Kennedy Space Center), 2022: about 50 m2 of crops feeds one person, about 20 m2 of plants gives one person's oxygen. NASA's Biomass Production Chamber had 20 m2 of growing area under 96 x 400 W high-pressure sodium lamps (about 38 kW of lamps for 20 m2; that is 1.9 kW per m2 of lamps, my arithmetic).
- Source: (a) Supplemental Food Production With Plants: A Review of NASA Research, Frontiers in Astronomy and Space Sciences, November 2021, frontiersin.org/journals/astronomy-and-space-sciences/articles/10.3389/fspas.2021.734343/full. (b) Bioregenerative Life Support for Sustainability in Space, R. M. Wheeler, USC seminar, April 2022, ntrs.nasa.gov/api/citations/20220006578
- Quotation: (a) "About 40-50 m2 of crops grown under high light intensities would be needed to produce enough dietary calories for one human" (b) "Approximately 50 m2 of plants (crops) can provide food for one human"
- Read: 2026-11-12
- Confidence: high (NASA researchers; agree with BVAD 65 m2)
- Use for the ship: 50 m2 lit area per person for all calories (65 m2 BVAD with a wider diet). Against the soil figure (372 m2), lit farming uses about 7 times less floor but needs power for the lights.

### C20. Plant-factory yields
- The number(s): leafy lettuce 60 to 105 kg fresh per m2 per year, 0.08 to 0.13 kg per kWh, one Japanese review about 91 kg per m2 per year (search summaries of PMC and review papers). No published figure found for the lit area one person needs for a complete diet from a commercial plant factory: they grow lettuce, which carries few calories.
- Source: search summaries of PMC10276559 and an ijabe.org review, not read directly
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: low
- Use for the ship: do not size a calorie farm from plant-factory lettuce; use C18 and C19.

### C21. Small livestock outdoor area (hens and a dairy goat)
- The number(s): hen: indoor 3 to 4 ft2 (0.28 to 0.37 m2), outdoor yard 10 ft2 (0.93 m2). Dairy goat: enclosed housing 20 to 25 ft2 (1.9 to 2.3 m2), exercise yard 50 ft2 (4.6 m2), pasture 0.2 to 0.3 acre (809 to 1,214 m2). For 4 hens and 1 goat: coop 1.1 to 1.5 m2, hen yard 3.7 m2, goat house 2 m2, goat yard 4.6 m2, plus 809 to 1,214 m2 pasture if the goat grazes. (A goat is a herd animal; welfare is another helper's job.) Other extensions give 25 ft2 exercise (UMass, NMSU); Mississippi State flags its page as outdated.
- Source: Housing and Space Guidelines for Livestock, University of New Hampshire Extension, extension.unh.edu/resource/housing-and-space-guidelines-livestock, created June 2009, updated March 2023
- Quotation: Dairy goat "20- 25 sq. ft.", "50 sq. ft.", "0.2-0.3 acres"; Hen "3- 4 sq. ft.", "10 sq. ft."
- Read: 2026-11-12
- Confidence: medium-high (one university extension, agrees with Purdue per search summary)
- Use for the ship: animal buildings and yards need under 15 m2; the pasture is the cost (about the size of the whole garden), so a homestead goat eats stored hay from a shared farm or a fodder plot.

## 6. Tatami

### C22. Standard tatami sizes
- The number(s): Kyoma 955 x 1910 mm (1.824 m2); Chukyoma 910 x 1820 mm (1.656 m2); Edoma 880 x 1760 mm (1.549 m2); Danchima 850 x 1700 mm (1.445 m2). A six-mat room is 10.9 m2 in Kyoma and 8.7 m2 in Danchima.
- Source: Japanese real-estate glossaries and an English calculator site (via search summaries and the mec-h.com and smtrc.jp glossary pages); NO Japanese government or JIS source found.
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: medium (many sources agree; none official)
- Use for the ship: recommend one module of 900 x 1800 mm = 1.62 m2 (my rounding between Chukyoma and Edoma). It is the same 1.62 m2 as the advertising rule in C23, so a room labelled "6 tatami" is 9.72 m2 on the ship and in a Japanese listing.

### C23. Does Japanese law size rooms in tatami
- The number(s): the tatami-count rule is in real-estate advertising, not in the Building Standards Act: a room shown in tatami must be at least 1.62 m2 per mat (wall-centre area divided by mats), Enforcement Rules of the Fair Competition Code on Real Estate Advertising, Article 11, item 16; the code dates from 1963 per a glossary.
- Source: 不動産の表示に関する公正競争規約施行規則, 不動産公正取引協議会連合会 (Real Estate Fair Trade Council), rftc.jp/webkanri/kanri/wp-content/uploads/2025/08/20250826_kiyakushuu.pdf, August 2025 edition. The PDF text layer lost the Japanese; the number "(16) 1.62" is visible but the sentence is not, so the wording is from search-result paraphrase.
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: medium
- Use for the ship: NEGATIVE FINDING: using tatami as a module does not make a design "to code". The 1.62 m2 rule is an industry advertising code, not a building code. I did not find a Building Standards Act article that requires tatami sizes (searches did not confirm either way). Safety floors come from C5 and C11, not from tatami.

## Recommended 5-tier ladder (all my arithmetic from the findings above; the household of 2.5 people is my assumption)

| Tier | People per unit | Private floor per person | Shared floor per person | Gross building per person | Site per person (drum floor) | Whole drum if all this tier |
|---|---|---|---|---|---|---|
| 1 Barracks (open bay, giant studio) | 100 to 400 per building, use 200 | 6.7 m2 net sleeping minimum (C1); recommend 9 m2 so it feels "excess" | about 11 m2 of dining, workshop, hall, baths (my allowance) | 20 m2 (13 m2 UFC cap for the bay building plus halls) | 20 m2 on one storey, 10 m2 on two; the garden is tended by all and is outside this figure | 157,000 people (one storey), 314,000 (two) |
| 2 Dormitory | 150 to 500 beds, use 250 | 8.4 m2 single room (C8, low confidence) | about 22 m2 of kitchens, baths, lounges, study (31 minus 8.4 and rounded) | 31 m2 (C7) | 31 m2 one storey, 15.5 m2 two | 101,000 (one), 203,000 (two) |
| 3 Co-housing cluster | 24 to 36 households (C9), about 60 to 90 people | 28 m2 (home of 70 m2, 2b3p two-storey, shared by 2.5 people, C11) | 5.6 m2 (common house 14 m2 per household, C10) | 33.7 m2 | about 88 m2 (a 220 m2 site per household, Marmalade Lane figures, low confidence) | about 35,700 people |
| 4 Row house | 4 people per house (a row of 6 to 12 houses) | 19.8 m2 gross (79 m2 GIA, 2b4p, C11) | none inside; street and yard are outside | 19.8 m2 | plot 5 m x 20 m = 100 m2 = 25 m2 per person (house footprint 40 m2, yard about 40 m2 per C13, front and side the rest); 2 storeys already assumed | 125,700 people |
| 5 Homestead | 1 person (a couple or a family of 4 is the same plot times 1 to 4 for soil only) | 39 m2 house (1b1p, C11), 50 m2 with porch and storage | all of it is private; trade goes through the market square | 39 to 50 m2 | 1,156 m2 plot (operator's 34 x 34 m): soil beds 372 m2 (C14), orchard and fodder about 500 m2, hens and goat yards about 15 m2, house footprint 50 m2 | about 2,600 homesteads |

Each tier's sector of 500 people at the site figure above covers: barracks 1 ha (one storey), dormitory 1.55 ha, co-housing 4.4 ha, row house 1.25 ha, homestead 60 ha (500 homesteads) or 0.6 km2.

Food ladder (floor that grows each person's diet, any tier): soil 372 m2 per person (Ecology Action) or 186 m2 expert; lit 50 to 65 m2 per person (NASA, C18, C19). Barracks and dormitory eat from the shared farm: 500 people need 18.6 ha of soil or 2.5 to 3.3 ha of lit area. Co-housing and row houses grow a kitchen garden and buy staples. The homestead is the only tier that carries its own full soil plot.

Noteworthy: the ladder is not monotonic in building area (barracks 20, row house 20, dormitory 31, co-housing 34, homestead 39 to 50), because crowding spares a large kitchen and hall by sharing, while a row house spends 19.8 m2 per person on private rooms. The big jump is in site: 10 to 25 m2 per person from barracks to row house, 88 for co-housing, 1,206 for the homestead. A person who wants the homestead needs about 50 times the floor of a row house.

## Could not read
- spherestandards.org (the Sphere Handbook): robots.txt disallows ClaudeBot and anthropic-ai (Disallow: /). Not read; UNHCR page used instead.
- wbdg.org FC 4-721-10N (2022): stg.wbdg.org returned 503; the S3 copy returned 403.
- online.ucpress.edu (Elementa journal page for Peters 2016): 403. Abstract read from Allegheny College repository.
- doaj.org (Peters abstract record): 403.
- openknowledge.fao.org (FAO cropland brief): 403.
- lists.cohousing.org (Cohousing-L archive): robots/403; only search-result summaries.
- ppdo.ucsc.edu housing standards PDF: 404.
- wisconsin.edu 12A3G residence hall study: over the 10 MB fetch limit.
- housinglin.org.uk Insights from Marmalade Lane PDF: over the 10 MB fetch limit.
- static1.squarespace.com GBSustainability Protocol 2018: not fetched (redirect; the figure was confirmed elsewhere).
- rftc.jp rules PDF: read but Japanese text not extractable.
- ntrs.nasa.gov BVAD Rev 2 and Wheeler PDF: read from saved copies with pdftotext (exact table rows, slide text).

## Negative findings and gaps
- No readable UK MoD JSP 315 room-size table; the SLA Design Guide has no m2 (C6).
- No Army regulation (AR 420-1 Table 3-10) or Army IET standard design read; the 72 ft2 is from the Navy UFC and an Army medical training text. A 2025 Army barracks memo (IPIMT72) caps open bays at 60 people with a 72-person surge: that 72 is people, not square feet.
- No university space standard for assignable ft2 per bed was readable; only the CPM gross survey (C7).
- No primary source for co-housing common-house or home size; mailing-list posts only (C10). The Cohousing Association of the US is now the Cohousing Alliance (cohousing.org redirects to cohousingalliance.org); I did not read its site.
- No published source for 34 x 34 m or 1,156 m2 as one person's year-round garden, and none for "a quarter acre" in a primary source (C17).
- No commercial plant-factory figure for lit area per person for a complete diet (C20).
- No Japanese government or JIS source for tatami dimensions; no confirmed building-law statement either way on tatami (C22, C23).
- The BVAD all-crops diet is not 100 percent of food (resupplied herbs, condiments); the 2022 BVAD Table 4-92 soybean row (46 m2) drives the 65 m2 total, so a ship diet with less soybean would need less.

## Summary table
| Question | Recommended number | Findings | Confidence |
|---|---|---|---|
| Barracks sleeping floor | 6.7 m2 net minimum, 13 m2 gross per person (bay building); use 9 net, 20 gross with halls | C1, C2, C3 | high (floor), medium (recommendation) |
| Absolute floor | 4.5 m2 indoor living space per person (3.5 m2 warm) | C5 | high |
| Barracks ladder inside military standards | 13 / 16.6 / 24 / 33 m2 gross per person (open bay, 2+2, 2+0, 1+1) | C1, C2 | high |
| UK MoD SLA | no figure found | C6 | n/a |
| Dormitory | 31 m2 gross per bed; 8.4 m2 private room | C7, C8 | medium / low |
| Co-housing | 24 to 36 households; 14 m2 common house per household; home 70 m2 | C9, C10, C11 | high (size), low (common house) |
| Row house | 79 m2 for 4 people (19.8 each); plot 5 x 20 m, 25 m2 per person | C11, C12, C13 | high (floor), medium (plot) |
| Soil food area | 372 m2 per person (186 expert) | C14 | high (their claim) |
| Field-farm land | 1,300 to 10,800 m2 per person per year | C15 | high |
| 34 x 34 m garden source | none found; about 3 times Ecology Action | C17 | n/a |
| Lit growing area, full diet | 50 m2 (NASA), 65 m2 (BVAD all crops); 19.5 m2 gives about half | C18, C19 | high |
| Plant factory | no calorie-complete figure | C20 | low |
| Hens and goat outdoor area | hens 0.93 m2 yard each; goat yard 4.6 m2, pasture 809 to 1,214 m2 | C21 | medium-high |
| Tatami | 900 x 1800 mm = 1.62 m2 module; Kyoma 955x1910, Edoma 880x1760, Danchima 850x1700 | C22, C23 | medium |
| Tatami and law | NOT a building-code module; 1.62 m2 is an advertising rule | C23 | medium |
