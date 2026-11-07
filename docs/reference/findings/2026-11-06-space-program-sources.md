# How much room does each person need aboard? Sources for the mothership's space program

**Research date: 2026-11-06.** Every source below was read on 2026-11-06 unless a finding says
otherwise. This is a reading of public sources, done to give the game honest numbers. **It is not
architectural, planning or engineering advice**, and nobody who wrote it is an architect, a planner
or an engineer. Anyone sizing a real building or a real habitat should go to the primary sources
and to a professional.

## The question

Where can the mothership's space program (`data/ship/space_program.toml`) get its area per person
for every kind of space, homes, civic and commercial places, schools, medical care, farms, life
support, storage, industry and transit, and the allowances for circulation and structure, so that
the ship's total size follows from its population?

Asked by the operator on 2026-11-06: "How do we end up measuring the total size of the ship? Do we
need to plot the size for each home, hangar, engineering bay, industrial zone, reactor room,
bridge, storage areas, transportation like railways and elevetors, etc?" The design that uses the
answer is [mothership-master-plan.md](../../design/mothership-master-plan.md), section 3.12.

## Short answer

- **Yes, bottom up, and NASA did exactly this in 1975.** NASA SP-413, *Space Settlements: A Design
  Study* (1977), sized its 10,000-person settlement from a table of space uses, each with its area
  per person, its number of storeys and its height, drawn from US building standards and community
  planning literature (S1 to S9). Its totals: **47 m2 of projected area per person for the
  community** and **20 m2 more for agriculture**, 67 m2 in all; 155.2 m2 of floor per person once
  the storeys are counted.
- **Its rows give most of the program**: residential 49 m2 a person of floor (12 projected, four
  storeys); shops 2.3; offices 1; schools 1; hospital 0.3 (a 50-bed hospital at 58 m2 a bed);
  assembly 1.5; recreation 1; public open space 10; service industry 4; storage 5; transportation
  12 (a quarter of a US city's, because there are no cars); waste and water 4; electrical 0.1;
  communications 0.05; miscellaneous 2.9.
- **Circulation inside buildings**: the US General Services Administration's planning guide says
  circulation multipliers "will typically range between 1.4 to 1.6" over the net room areas (S10).
  NASA's figures already include access and walkways, so the multiplier applies only to rows given
  as bare rooms.
- **Farms**: this project's 2026-10-03 finding already holds the sourced figures (S11): NASA's
  2022 baseline needs 65.29 m2 of growing area per crew member for its most closed diet; sunlit
  biointensive beds need 232 to 372 m2.
- **Not found**: a sourced floor area for hangars, craft cradles, a ship's bridge, a reactor
  pair's halls or heavy industry in a self-sufficient settlement, or a structure allowance. The
  program carries those as Proposals, labelled so.

---

## How the sources were read

- NASA SP-413 was downloaded from NASA's Technical Reports Server
  ([ntrs.nasa.gov/citations/19770014162](https://ntrs.nasa.gov/citations/19770014162), the record
  dated "January 1, 1977", report number NASA-SP-413, edited by Richard D. Johnson of Ames Research
  Center and Charles Holbrow of Colgate University), as
  `https://ntrs.nasa.gov/api/citations/19770014162/downloads/19770014162_update.pdf` (231 MB, a scan
  with a text layer), and turned into text with `pdftotext -layout`.
- **The scan's text layer has optical character recognition errors**: stray spaces inside words
  and numbers ("m 2 /person", "9 15 m3"). Quotations below remove those stray spaces and change
  nothing else. Each quotation was checked against a second reading: the National Space Society's
  transcription of the same study (settlement.nss.org/75SummerStudy/), whose Table 3-2 is an image
  of the printed table, read as an image. The two agree on every number quoted.
- Area units: m2 is square metres, m3 cubic metres. "Projected area" is SP-413's term for the floor
  a space covers once it is stacked into storeys (S1).

---

## Findings

### S1. Area per person is the criterion, measured as projected area

NASA SP-413, chapter 3 ("Human Needs in Space"), page 24, "Space Needs Within the Colony" (1977):

> "A brief survey of the literature (refs. 40-43) indicates that there should be at least 40 m2 of
> projected area per inhabitant. Projected area means area projected onto the largest plane
> perpendicular to the direction of the pseudogravity. Thus a three-story house with 60 m2 of
> floor space occupies only 20 m2 of projected area."

Its Table 3-1 ("Available land per capita in selected cities and towns") sets that against Earth:
Boston 185.8 m2 a person, New York 98.3, Manhattan Borough 38.2, Rome 40.0, Soleri's Babel IIB
15.1, and "Space colony" at "≥40.0" (read from the table image; the text layer scrambles its
columns).

### S2. NASA's totals: 47 m2 of community and 20 m2 of agriculture per person

NASA SP-413, page 25:

> "The results of this study are summarized in table 3-2. They suggest that, to meet community
> needs, 47 m2 of projected area and 823 m3 of volume are needed for each inhabitant. Agriculture
> requires an additional 20 m2/person and 915 m3/person."

### S3. Table 3-2, the space program itself

NASA SP-413, page 26, "Table 3-2. Summary of community space and area allocations". The rows as
printed (surface area in m2 a person, number of levels, projected area in m2 a person, estimated
height in m, volume in m3):

| Space use | Surface area | Levels | Projected | Height | Volume |
|---|---|---|---|---|---|
| Residential | 49 | 4 | 12 | 3 | 147 |
| Shops | 2.3 | 2 | 1.0 | 4 | 9.2 |
| Offices | 1 | 3 | .33 | 4 | 4.0 |
| Schools | 1 | 3 | .3 | 3.8 | 3.8 |
| Hospital | .3 | 1 | .3 | 5 | 1.5 |
| Assembly (churches, community halls) | 1.5 | 1 | 1.5 | 10 | 15 |
| Recreation and entertainment | 1 | 1 | 1 | 3 | 3 |
| Public open space | 10 | 1 | 10 | 50 | 500 |
| Service industry | 4 | 2 | 2 | 6 | 24 |
| Storage | 5 | 4 | 1 | 3.2 | 16 |
| Transportation | 12 | 1 | 12 | 6 | 72 |
| Communication distr. switching equipment for 2800 families | .05 | 1 | .05 | 4 | .2 |
| Waste and water treatment and recycling | 4 | 1 | 4 | 4 | 16 |
| Electrical supply and distribution | .1 | 1 | .1 | 4 | .4 |
| Miscellaneous | 2.9 | 3 | 1 | 3.8 | 11.2 |
| Subtotals | 94.2 | | 46.6 | | 823.3 |
| Plant growing areas | 44 | 3 | 14.7 | 15 | 660 |
| Animal areas | 5 | 3 | 1.7 | 15 | 75 |
| Food processing, collection, storage, etc. | 4 | 3 | 1.3 | 15 | 60 |
| Agriculture drying area | 8 | 3 | 2.7 | 15 | 120 |
| Totals | 155.2 | | 67.0 | | 1738.3 |

Its note: "The areas and volumes arrived at are approximations for use in establishing mass
estimates and aiding in the structural design of the entire habitat enclosure."

### S4. Homes: 49 m2 a person, from the building code

NASA SP-413, chapter 3, Appendix B ("Space Requirements of Various Community Activities"),
page 32:

> "Minimum room sizes for residences are determined from the Uniform Building Code for residential
> occupancies. An area of 148 m2 for a family of four satisfies the requirements of the code
> comfortably. When space for external use and access is included, the following recommended
> minimums result: 37 m2/person of floor area, 12 m2/person of exterior space, totaling
> 49 m2/person of total residential space."

And: "For the residential areas the stacking factor is taken to be 4, thus the required projected
area is a little more than 12 m2/person."

How Appendix B was built, page 32: "the study group used U.S. building standards and estimations
of allocation of land use found in the extensive literature of community planning (ref. 46)."

### S5. Shops and offices

NASA SP-413, Appendix B, page 32:

> "Area for shops is determined from recommendations of the Town Planning Committee of South
> Australia which calls for 10 shops per 1000 persons, each with a floor space of 1 m2/person, an
> area for walkways and access of 1 m2/person, and 0.3 m2/person for expansion."

> "In a similar way the same source is used to estimate an area of 1 m2/person for office space."

### S6. Schools and the hospital

NASA SP-413, Appendix B, pages 32 to 33:

> "Adopting the highest value recommended, 10 m2/person, and assuming 3-story schools, leads to
> 3.3 m2/student. If student population is 10 percent of the total population, the required
> projected area is 0.3 m2/person."

> "A 50-bed hospital including administrative, diagnostic, treatment, nursery, surgical,
> obstetrical, service, and circulation facilities requires 58 m2/bed or 2900 m2 total."

(The 50 beds: 28 scaled from a recommendation of "693 beds for 250,000 people", raised because
"Because of the nature of the colony and its isolation, a more realistic number might be 50.")

### S7. Assembly, entertainment and open space

NASA SP-413, Appendix B, page 33:

> "DeChiara and Koppelman specify 1.5 m2/person for 10,000 people for general community facilities
> such as churches, community halls, and theaters."

> "To include all commercial entertainment, 1 m2/person is assumed."

> "Averaged over 53 U.S. cities the open space for parks and such is 18 m2/person. DeChiara and
> Koppelman recommend 14 m2/person. Because the space habitat contains agricultural areas that can
> be in part used as open space, a lower value of open space in the residential area is adopted,
> namely 10 m2/person."

And on its height: "To allow a true feeling of being "open" the space has to be tall enough. This
height is taken to be 50 m."

### S8. Light industry, storage and the mechanical subsystems

NASA SP-413, Appendix B, page 33:

> "For the planning of new towns the Town Planning Committee of South Australia recommends
> 4 m2/person for light service industries. An average over 53 U.S. cities gives 8 m2/person. For
> planning purposes in the colony 4 m2/person is assumed."

On storage: "The colony has 5 m2/person alloted to these purposes" (wholesaling and storage).

> "By analogy with Earth-like situations, a total of 400 m2 is allocated to communication
> distribution and switching equipment for 2800 families, 40,000 m2 for waste and water treatment
> and recycling, and 1000 m2 for electrical supply and distribution, a total of about 4.2
> m2/person."

### S9. Transportation: a quarter of a city's, because there are no cars

NASA SP-413, Appendix B, page 33:

> "About 24 percent of the total land use in U.S. metropolitan areas is devoted to transportation,
> that is, approximately 48 m2/person (ref. 48). However, Earth-like streets are not needed in the
> compact, closely-knit organization of the colony."

> "Thus, only one fourth as much area need be allocated to transportation in the colony as on
> Earth. The numerical value is 12 m2/person."

> "Because of the relatively high population density (15,000 people/km2) in the community, most of
> the circulation is pedestrian, with one major mass transport system (a moving sidewalk, monorail,
> and minibus) connecting different residential areas in the same colony."

### S10. Circulation inside buildings: a multiplier of 1.4 to 1.6

U.S. General Services Administration, Public Buildings Service, *Circulation: Defining and
Planning*, dated "May 11, 2012" on every page, published on the Whole Building Design Guide
([wbdg.org, file Circulation_-_Defining_and_Planning.pdf](https://www.wbdg.org/FFC/DOD/UFC/600SERIES/RESOURCES/Circulation_-_Defining_and_Planning.pdf)).
It is about offices.

The method: the planner itemizes space types, "The subtotal of this calculation, also known as Net
Area, is then adjusted to take into account pathways leading to and in-between the different types
of spaces. This is accomplished by applying a Circulation Multiplier, which takes the form of 1.##
(e.g., 1.50), to the Net Area."

> "Circulation Multipliers will typically range between 1.4 to 1.6. As the portion of open
> work-spaces increases, the Circulation Multiplier will also correspondingly increase."

> "As a general planning "rule of thumb," Circulation Area comprises roughly 25 to 40% of the total
> Usable Area."

And the warning about the old default: "Historically, a Circulation Multiplier of 1.35 applied to
the Net Area was assumed to provide adequate space for circulation, regardless of the
configuration of the building or the type and ratio of open and enclosed spaces. However, when this
method is used, the resultant Circulation Area is just 26% of the total Usable Area."

### S11. Farms: area per person fed (not re-read today)

[2026-10-03-food-land-and-transport-speeds.md](2026-10-03-food-land-and-transport-speeds.md), whose
quotations were read and re-checked on 2026-10-03, holds the sourced figures; they were not read
again today. In short: NASA's Baseline Values and Assumptions Document (2022) needs 65.29 m2 per
crew member for its most closed diet (F10); NASA's colony planning figure is about 50 m2 (F9);
Lunar Palace 365 grew 73% of four people's food by dry weight on 30 m2 each (F12); sunlit
biointensive beds need 232 to 372 m2 a person (F2, F3). SP-413's own plant growing area, 44 m2 a
person on three levels (S3), sits between the lamp-lit and the sunlit figures.

The homestead engineer's survival garden work (2026-11-06, in progress: `docs/design/survival-garden.md`)
sets the garden's goal, in the operator's words "a minimalist survival garden that provides all the
nutrients it possibly can", and will refine the farm area once its diet is worked out.

### S12. The ship's own budget per person (project document)

[closed-loop-test.md](../../design/closed-loop-test.md), the project's closed-loop budget with its
own sources: 10.8 kW of electricity a person, a 60 MW net reactor pair of the game's KLT-40S class,
65 m2 of growing floor with crew access, and 67 m2 of radiator fin a person. A project document,
not an outside source; listed so the program's rows can name it.

---

## What is still unknown, and what would settle it

| Unknown | What would settle it | Cost |
|---|---|---|
| Floor area of a hangar bay, a craft cradle, a mech bay, a drone bay | The transport craft and the private craft as design files (game-setting-and-intro.md plans the transport); the bays sized from their dimensions plus working clearance | A design pass, no research |
| Floor area of a reactor pair's halls | A published general arrangement of a KLT-40S class plant (the IAEA's Advanced Reactors Information System holds design descriptions) | An hour of reading |
| Heavy industry per person in a settlement with no Earth to trade with | SP-413's settlement kept heavy industry outside the habitat. A survey of industrial floor area per person in self-sufficient economies, or the closed-loop work's materials loop | A research pass |
| Circulation inside apartment blocks (the GSA figures are for offices) | A residential planning guide's net-to-gross ratio for flats | An hour of reading |
| A structure allowance (walls, columns, shafts) over usable area | A floor measurement standard's typical gross-to-net ratios (BOMA, IPMS) | An hour of reading; the standards may be sold |
| Farm area for the ship's own diet under the sunline | The survival garden design (in progress) | Under way |

## Sources that could not be reached

- `https://ntrs.nasa.gov/api/citations/19770014162/downloads/19770014162.pdf` returned 404; the
  record's own link (`..._update.pdf`) worked.
- `https://settlement.nss.org/75SummerStudy/Chapt.3.html` returned 404 (the right page is
  `Chapt3.html`). The transcription's tables are images, not text.
- Nothing else was refused.

## What it means for this project

### Certain (from the sources)

- Sizing a settlement by summing area per person over a list of space uses is how NASA's 1975
  study did it, from US building standards and community planning literature (S2 to S4).
- NASA's community rows total 47 m2 of projected area per person and its agriculture 20 m2 more
  (S2), at a settlement density of 15,000 people per km2 (S9).
- Office planners add 40 to 60% to net room areas for circulation (S10).
- NASA's figures already include access and walkways for homes and shops (S4, S5), so adding a
  circulation multiplier to them would count circulation twice.

### Judgement calls (ours; a reader may weigh them differently)

- The program uses SP-413's rows unchanged where the mothership has the same need (homes aside,
  every community row), because they are the one complete, sourced, settlement-scale table, and
  marks every other row a Proposal.
- Homes come from this project's home kinds (the homestead plot, the cabin, the atrium apartment
  block) rather than SP-413's 49 m2, because the operator chose the kinds (2026-10-03) and a
  homestead is a farm as well as a home.
- Farms use NASA's 2022 figure (65.29 m2 a person fed) on one level under the sunline, rather than
  SP-413's 44 m2 on three, because the 2022 figure is newer, measured for a closed diet, and the
  sunline lights one level best; stacking is the lever when the floor runs short.
- The circulation multiplier is the low end of the GSA's range, 1.4, because the ship's buildings
  are mostly enclosed rooms, which the GSA says need less.
- SP-413 set its rotation criterion at "≤1 rpm" (Table 3-3, read from the table image); Sector 1 spins at 1.89 rpm (the
  operator decided its size on 2026-09-15; [habitat-generation.md](../../design/habitat-generation.md)
  notes the spin is "under the ~2 rpm Coriolis comfort limit", a design note, not his words). Later research on rotation comfort
  was not read for this finding; it is a question for the crew-health work, not a reason to reopen
  the size.

This is a reading of public sources, not architectural, planning or engineering advice.
