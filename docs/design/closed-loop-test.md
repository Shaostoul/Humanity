# Closing the loops, from one person to all of humanity: the test plan (2026-10-23)

The operator, 2026-10-22: "Let's create a test for establishing as close to 100% closed loop
self-sustaining as possible for 1 person up to 100% of Humanity. We have two metrics that really
matters. Earth, today, real, now. Then for the video game the main mothership housing billions of
humans across interstellar space. Starting limited by the technology we have today. Stuff in R&D
that have promising results we can consider. Like the fusion reactors we're inventing. ... We need
to think like 1,000 years from now we're still going to be living and doing what we gotta do."

This is the whole plan, written before anything is built, per the house rule for features: plan
every part first so the data shapes, the compute budget and the scale are decided once, and the
first slice built is a rung of the real thing. **Nothing here is built yet.** The numbers are in
`data/closed_loop/` (eight files, shape in `schemas/closed_loop.toml`), each with its source, the
source's own date, the date it was read (2026-10-23 unless marked), and the result of trying to
prove it wrong. Phase 2 builds the test on the game's real systems; its work list is section 13.

**Second review, 2026-10-23.** An independent critic re-checked the figures against primary
sources the same day and found errors the first pass's own refutation had missed, the worst a
reactor that could not exist and a phosphorus loss overstated about 2.4 times. Each finding was
re-read here before it was applied; section 11.2 lists what changed, what was disputed, and why.
Its main point is now a rule of the test: a case that re-derives a figure's own arithmetic cannot
catch a wrong input, so every load-bearing figure also has a case that checks it by an
independent route (section 8.2).

Read with: [ship-life-support.md](ship-life-support.md) (the homes' measured air and water),
[homestead-solo-design.md](homestead-solo-design.md) and `data/home_outline.json` (one person, done
properly), [headless-sim-runner.md](headless-sim-runner.md) (the runner this test runs on),
[offline-progression.md](offline-progression.md) (the jump through time), and
`data/self_sufficiency/cannot_close.ron` (the five loops one home never closes).

This is a reading of public sources for a game and a teaching tool. It is not engineering advice
for anyone building a real closed habitat.

---

## The short answer

**Earth today closes most of its loops by being a planet, and almost none of the loops people
added on top.** Of what humanity emits, about half the carbon comes back to land and ocean over a
decade (a third in 2024); 8.6% of its traded energy comes from flows that refill (18% of final
consumption counting firewood and dung); 6.9% of the materials it uses are recycled; its
phosphorus is mostly mined, with no substitute. Water closes for the planet but not where people
live. Food is regrown every season but about 30% of it is lost or wasted.

| Loop | What the fraction is a fraction of | Earth today | Data year |
|---|---|---|---|
| Air (carbon) | Human CO2 taken back up by land and ocean | **about 50%** over a decade; **32%** in 2024 (emissions less what the air kept) | 2015-2024 |
| Water | Household wastewater safely treated | **56%** (one source); withdrawals are 9.4% of renewable freshwater (one undated denominator) | 2024, 2023 |
| Food | Food grown that is not lost or wasted | **about 70%** (loss by value, waste by mass: approximate) | 2022-2023 |
| Nutrients | Phosphorus put on land that is recycled, not mined | **about 43%** (mostly manure); 1.4 to 2.4% of fertilizer phosphorus comes back from people | 2005 |
| Energy | Energy from sustained flows | **8.6%** of commercially traded supply (86% fossil); 18.0% of final consumption, 13.4% without traditional biomass | 2025, 2023 |
| Materials | Materials entering the economy that are recycled | **6.9%** | 2021 |
| Habitat | Cropland per person | **0.19 ha (0.47 acre)**, falling | 2023 |

**The mothership closes its air, water and food with technology that operates today, at a
price set by light and by heat.** Every person needs about 50 m2 (540 sq ft) of crops lit day and
night, about 9 kW of electricity for those lights at fixtures demonstrated today, and about 10.4 kW
in all. For 8.2 billion people that is 85 TW: twenty-three times all the electricity Earth makes,
four and a half times its whole energy supply. And the reactors that make it waste more than they
deliver: the game's reactor class turns 23.3% of its heat into electricity, so each person's 10.4 kW
comes with 34 kW of waste heat, and every watt of both must be radiated into space. That takes about
56 m2 (600 sq ft) of radiator fin per person, 2.9 times the hull's whole side: **fins the size of
the hull's side serve about 2.8 billion people, a third of the 8.2 billion.** The loops that decide
whether the ship survives a thousand years, besides heat, are **nutrients** (the one crewed closed
system whose mineral retention this pass could read, BIOS-3, kept about 20% of its minerals on
average, by its own design; the ship needs 99.9%) and **materials** (nothing new arrives between the
stars).

| Per person aboard, today's technology | Per person | All 8.2 billion |
|---|---|---|
| Lit crop canopy | 50 m2 (540 sq ft), 35 to 56 | 410,000 km2 (158,000 sq mi) |
| Growing floor with crew access | 65 m2 (700 sq ft), 44 to 84 | 533,000 km2 (206,000 sq mi) |
| Electricity, all uses | **10.4 kW**, 250 kWh a day (4.3 to 19) | 85 TW |
| ... of which lighting the crops (2.8 umol/J fixtures) | 9 kW (3.5 to 16) | 74 TW |
| Reactor waste heat (KLT-40S class, 23.3%) | 34 kW | 280 TW |
| Radiator fin: habitat at 290 K + reactor waste at 300 K | **56 m2** (600 sq ft): 14 + 41 | 457,000 km2, 2.9 times the hull's side |
| Oxygen | 0.62 to 0.76 kg (1.4 to 1.7 lb) a day, made by the crops | |
| Phosphorus lost at an assumed 20% / 99% / 99.9% retention | 0.9 kg / 9 g / 0.9 g a year | 7.2 million t / 90,000 t / 9,000 t a year |
| Reactor fuel, fission (23.3%) / fusion carried (assumed 33%) | 17 kg / 3.7 kg a year | 140,000 t / 30,000 t a year |
| Shield over the whole hull at 4.5 t/m2 | 95 t (105 short tons) | 780 billion t |

**The research options that matter most**, in order of what they change (section 7):

1. **Food per kilowatt-hour.** Lighting is 87% of the ship's electricity, and through the reactors
   most of its heat and radiators. LEDs at their calculated limits save 18% (3.4 umol/J) to 32%
   (4.1) against today's demonstrated 2.8. The next step is food that skips photosynthesis:
   protein from hydrogen-eating bacteria (a commercial plant on Earth, readiness 8 there and 4
   aboard; a derived 31 to 34 kWh per kg, against a 2017 laboratory figure of 24) and food grown on
   electrolytic acetate (laboratory, readiness 3 to 4).
2. **Heat rejection and conversion efficiency.** A 23.3% plant rejecting at 300 K needs three times
   the radiator for its own waste heat as the habitat needs for its electricity. A hotter
   condenser, a more efficient plant or radiators beyond the hull change how many people the ship
   can carry; none is sourced here.
3. **Mineral recovery.** At BIOS-3's 20% (an all-mineral average, assumed for phosphorus) a ship of
   8.2 billion would lose 5.4 to 9.0 million t of phosphorus a year, a quarter to two-fifths of
   Earth's fertilizer phosphorus. At 99.9% it carries about 9 million t for the whole voyage.
4. **Material recovery per product life.** At 95% recovery per 20-year life, 8% of a material
   survives a thousand years; at 99.9%, 95%.
5. **Fusion**, the operator's first named option, matters for the **fuel stock and the waste**: per
   joule it carries about 0.31 of fission's fuel mass (deuterium plus the lithium its tritium is bred
   from) and leaves no long-lived actinides. It does not change the habitat's heat, and its own
   plant's efficiency and waste heat are unproven. Fusion has ignited at the target (8.6 MJ from
   2.08 MJ of laser light, April 2025) but has not yet sent net electricity to any grid (readiness 3
   to 4).

**Three things in this repository did not balance**, and two were fixed (section 12):
`data/machines/home_solo.ron`'s Nutrients loop said it closed (it does not; the family home and
the outline already said so), and `data/home_outline.json` summed the bare-tier land to 0.34 ha
where its parts give 0.36. And **no single "percent closed" number by mass means anything**: a home
that recycles its water and air and grows no food at all is 99.1% closed by mass, because water
dominates the mass (section 1). The test refuses to print one.

---

## 1. What "closed" means, and the one-number trap

A loop is **closed** when what leaves it comes back: the carbon you breathe out is taken up by a
plant you later eat, the water you flush is cleaned and drunk again. It is **open** where something
must be brought in (an import) or is lost for good (a loss), and it **draws on a stock** where it
runs on a store that does not refill (fossil fuel, phosphate rock, a ship's fuel).

Closure is measured **per loop, by what passes through it**:

    closure of a loop = 1 - (imports to the loop) / (what the loop supplies)

over a stated period, by mass for the matter loops and by source for energy. Energy is the
exception: it is never recycled (every joule ends as heat, the second law), so its "closure" is the
share that comes from flows that refill (sunlight, wind, rivers) on Earth, and the years of fuel
carried against the voyage on a ship.

**Why one number is a trap.** One person's day, from NASA's current baseline (`person.ron`, from
bvad-2022 Table 3-31): 0.895 kg (2.0 lb) of oxygen and 0.80 kg (1.8 lb) of dry food in, about 4 kg
(9 lb) of water drunk or eaten, and in a comfortable home about 85 L (22 gal) of household water.
A home that recycles all of its water and air and imports **every meal** is

    1 - 0.80 kg of food / 87.5 kg of total throughput = 99.1% closed by mass.

Without the household water it is still 86%. So a headline like Lunar Palace 365's world-record
98.2% "material closure" (370 days, four people at a time, beihang-2026) is meaningful only when
it says what it is a fraction of, and its page does not. The test reports every loop separately,
and a test case (`closure_is_never_one_number_by_mass`) fails any report that prints an aggregate
without the table beside it.

---

## 2. The seven loops

They follow `data/home_outline.json`'s seven, with Materials made its own loop because the five
loops one home cannot close live there.

| Loop | What it covers | Main stocks | Main flows |
|---|---|---|---|
| **Air** | Oxygen, carbon dioxide, trace gases, nitrogen | the air's O2 and N2 | breathing, photosynthesis, leakage, scrubbers |
| **Water** | Drinking, hygiene, agriculture, recovery | tanks, cistern, the air's vapour | use, transpiration, condensation, treatment |
| **Food** | Calories, protein, micronutrients, crop area | silo, cellar, seed bank | growth, harvest, storage loss, waste |
| **Nutrients and waste** | Nitrogen, phosphorus, potassium, carbon, compost | soil, nutrient stores | uptake, excreta, residue return, export |
| **Energy** | Generation, storage, demand per person, heat | fuel, batteries | generation, use, waste heat, radiation |
| **Materials** | Manufacturing, repair, recycling, what cannot be closed | every material aboard | wear, recovery, dissipation |
| **Habitat** | Area, volume, shielding, health, the social side | the hull, the shield | dose, crowding, maintenance work |

Micronutrients (vitamin B12, iodine, the rest) are named in the Food loop and **not researched in
this pass**: `data/home_outline.json` closes B12 with fish, and phase 2 must source the full list
before the test claims a diet is complete.

---

## 3. The metrics

### 3.1 Closure fraction, per loop
`1 - imports / supply`, by mass, per day and per year. Each case also prints the imports and the
losses themselves, because a fraction hides scale.

### 3.2 The energy cost of closing a loop
What it costs to bring a kilogram back: about 6.5 kWh per kg of oxygen by electrolysis, 4.35 kWh
per kg of CO2 removed by the ISS sorbent bed (`technologies.ron`, machine_air), about 27 Wh of fan
per litre condensed out of a greenhouse's 75% air and 105 Wh out of a home's 50% air (the game's own
measurement, ship-life-support.md section 5). A loop that closes at a ruinous energy cost has moved
its gap into the Energy loop, and the test shows it there.

### 3.3 Imports and losses per person per year
In kilograms per person per year, by loop: phosphorus bought, water made up, nitrogen leaked, fuel
burned, spares used.

### 3.4 Area, volume and power per person
Crop canopy and floor (m2), living area (m2), volume (m3), electricity (kW and kWh a day), radiator
(m2). Dual units in every report.

### 3.5 Buffers, and failure margins
A **buffer** is a stock that covers a loop while it is broken: the air's oxygen, the cistern, the
silo. Its **duration** is the stock divided by the gross demand with the loop's regeneration gone.
A **failure margin** asks, for each single loss, which loop collapses and how long until it
matters. The solo home's, from this plan's hand estimates (phase 2 measures every one,
`time_to_harm_when_one_thing_fails`):

| Single loss | What collapses | How long until it matters |
|---|---|---|
| Every crop dies, scrubber off | Carbon dioxide | about 22 days to 2,636 ppm (the scrubber's setpoint, BVAD's coming crew limit), about 100 to 10,000 ppm: the resident and two mushroom tents breathe out about 1.97 kg (4.3 lb) a day into 11,160 m3 (394,000 cu ft), about 96 ppm a day |
| Every crop dies, scrubber on | Oxygen | the home air's about 3,100 kg (6,800 lb) of oxygen lasts about 13 years of breathing; people are harmed long before it is gone (the threshold is to be sourced) |
| Nothing fails, today | Oxygen, slowly | the home loses about 0.24 kg (0.5 lb) a day (0.1 the crops do not make back, 0.14 in leaked air, home_solo.ron): 88 kg a year |
| The air handlers stop | Water | the garden's breath is no longer returned: the cistern empties in about three to four weeks |
| The reactor tie is lost at night (Realistic mode) | Power | loads shed by priority, the CO2 scrubber last (tested today, ship-life-support.md section 2) |
| The harvest fails | Food | the silo's 750 days (catalogue figure) |
| The well and rain fail | Water | 8,000 L at 128 L a day net, about 62 days (home_solo.ron) |

The ship's air buffer is far larger: if the lore's whole hull holds air, it holds 133 t (147 short
tons) of oxygen per person, 480 to 590 years of breathing with no plants at all
(`ship.air.stock_if_whole_hull`). That is why the test watches **the loop and the stock together**:
Biosphere 2, with a far smaller but still generous buffer, lost a third of its oxygen in 16 months
to soil microbes and curing concrete, and no machine reported it (dempster-2008). The
`a_hidden_sink_is_found` case injects exactly that.

### 3.6 The report
One table per run: each loop's closure, imports, losses, energy cost, buffers and time to harm,
with the run's simulated time, step and measured step error. Never a single closure number.

### 3.7 Health and the social side, as far as they can be measured
Measurable proxies, not a model of happiness:

- **Space**: projected area per person (SP-413's literature survey: "there should be at least 40
  m^2 of projected area per inhabitant"; its design used 47 m2 for living plus 20 for farming) and
  volume per person.
- **Radiation dose** a year and over a life (behind 4.5 t/m2, 922 lb/sq ft, SP-413's 5 mSv a year,
  about 400 mSv a life; unshielded deep space 1.8 mSv a day, past NASA's 600 mSv career limit,
  kept in NASA-STD-3001 Volume 1 Revision C of 2023, in one year).
- **Gravity** (spin rate) and **light** (a day-night cycle for people as well as plants).
- **Work**: hours a person a week spent keeping the loops running (maintenance, farming), from the
  game's own task load once crew perform loops (the-mothership.md).
- **SP-413's qualitative criteria**, as a checklist the construction tool can report against: long
  lines of sight, natural light, "parts of interior out of sight of others", "availability of
  privacy", the ability to isolate one part of the habitat from another.

What the test will not pretend to measure: whether people are happy, or whether a society holds
together. It says so in every report.

---

## 4. From one person to 8.2 billion

`data/closed_loop/population_steps.ron` carries the ladder. In short (verdicts on Earth / aboard):

| Step | People | Air | Water | Food | Nutrients | Energy | Materials | What it unlocks |
|---|---|---|---|---|---|---|---|---|
| One person | 1 | closes / nearly | closes / closes | closes / partly | open / open | closes / open | open / open | nothing shared |
| Household | 2 to 8 | closes / nearly | closes / closes | closes / partly | open / open | closes / open | open / open | shared machines, shared heat |
| Village | 100 to 1,000 | closes / closes | closes / closes | closes / closes | open / partly | closes / open | open / open | a smithy, a midwife, a granary |
| Town | 5,000 to 50,000 | closes | closes | closes | partly | closes / partly | open | a clinic, metal casting from scrap, phosphorus recovery |
| City | 100,000 to 10 million | closes | closes | closes | partly | partly | partly | a hospital, steel from scrap, a chemical works |
| Nation | 10 million to 1.4 billion | partly / closes | partly / closes | closes | open / partly | partly | open / partly | pharmaceutical synthesis, some chip making |
| Humanity | 8.2 to 10.3 billion | partly / closes | partly / nearly | partly / closes | open / open | open / partly | open / open | every industry, including the chip supply chain |

The "one person" row is the game's measured homes aboard and the outline's homestead on Earth.
Above a household, **most thresholds are estimates and the file says so**: no source measures the
smallest town that can run a clinic. The thresholds that do have sources:

- **Genetics.** Marin and Beluffi found 98 people enough for a 6,300-year voyage in their Monte
  Carlo, which manages breeding every year, and say it keeps "a healthy population virtually
  indefinitely" (marin-2018). Smith's genetic review asks for 14,000 to 44,000 founders, 40,000 (an
  effective population of 23,400) as the safe figure, for health over five generations
  (smith-2014). Both claim health; they disagree by two orders of magnitude because their models
  and assumptions differ, which this pass cannot adjudicate. The test carries both.
- **Knowledge.** Henrich's model of Tasmania, cut off by the sea, has a small isolated population
  keeping simple skills and losing complex ones (henrich-2004); contested by later modelling.
- **Chips.** More than 50 points in the semiconductor supply chain where one region holds more than
  65% of world share, and all capacity below 10 nm in Taiwan (92%) and South Korea (8%); a
  self-sufficient chain in every region would need at least $1 trillion more and raise chip prices
  35 to 65% (bcg-sia-2021, an industry association's report). Electronics closes only at the scale
  of all humanity, and even then not in any one place.

**What changes with N.** Food, water, oxygen and phosphorus scale with the number of people.
These do not (`population_steps.ron` and data/home_outline.json's `scaling`):

- Heat loss follows a building's surface, about its volume to the two-thirds: sharing a roof is the
  largest single saving.
- Peak power grows roughly with the square root of N; trace contaminant control serves a cabin's air
  and what its equipment gives off, not a head count (data/home_outline.json), so its cost per person
  falls as a home fills; how it scales in a ship-sized volume is not sourced.
- **Industry is a step**: below a threshold the loop is open, above it closed.
- Genetic loss per generation is 1 / (2 Ne): negligible for a city, decisive for a village.
- Redundancy gets cheaper per person: one spare protects four people or forty thousand.
- Shielding per person falls as a habitat grows; **heat rejection does not**. Each person's 10.4
  kW, plus the 34 kW of waste heat the game's 23.3%-efficient reactors make delivering it, needs
  about 56 m2 of radiator fin, so fins as large as the lore's hull side (157,080 km2, 60,600 sq mi)
  serve about 2.8 billion people (3.9 billion with an unsourced 33% plant). **The ship is past its
  heat limit at 8.2 billion**, long before its space limit, unless its radiators reach well beyond
  the hull, its reactors reject heat hotter or more efficiently, or each person needs less.

**Earth, if everyone lived on a self-sufficient homestead.** The outline's homestead needs 0.28 to
0.36 ha (0.7 to 0.9 acre) a person at the bare tier (greenhouse 200 m2, arable 1,700 to 2,100,
woodlot 400 to 800, house and yard 500): 2,300 to 2,950 million ha for 8.2 billion people, under
all agricultural land (4,800). Heated by anything but a woodlot, 0.24 to 0.28 ha: 1,970 to 2,300
million ha. At the comfort tier, heated by a woodlot, 0.83 to 1.46 ha: 6,800 to 12,000 million ha,
more than all agricultural land, and at the top of the range 92% of all 13,000 million ha of land on
Earth. So the land needed is decided mostly by the house and how it is heated: the woodlot is most
of it, and dropping it is a technology choice (a heat pump, more insulation). This corrects the
first pass, which summed the bare tier to 0.34 ha, as data/home_outline.json did (now fixed), and
called self-sufficiency "a land question before a technology question", which the outline's own
note that the woodlot "disappears the moment you stop heating with wood" contradicts.

---

## 5. Scenario A: Earth, today, real, now

Every number below is in `earth_today.ron` with its quotation. Population 8.2 billion in 2024,
peaking near 10.3 billion in the mid-2080s (wpp-2024).

**Energy (2025, Energy Institute Statistical Review 2026).** Total energy supply 600.31 EJ (569
quadrillion Btu), an average of 19.0 TW: 72.9 GJ (69.1 million Btu), 2.3 kW, per person. Oil 200.97
EJ, gas 150.70, coal 166.03: 86.2% fossil. Nuclear 5.2%; hydroelectricity and renewables 51.57 EJ,
8.6%. Electricity 32,202 TWh, 3,930 kWh a person a year, 450 W each on average. **Scope**: the
review counts commercially traded fuels only, so the wood and dung burned outside markets are
missing, and it counts wind and solar at the electricity they make. Measured another way, SDG
indicator 7.2.1 puts renewables at 18.0% of total final energy consumption in 2023, 13.4% without
traditional biomass (sdg-2026-goal7). The test carries 8.6 to 18% and always prints which.

**Air (Global Carbon Budget 2025, NOAA).** Human emissions 11.6 GtC (42.4 Gt of CO2) in 2024, about
5.1 t (5.7 short tons) of CO2 per person; the land and ocean took up 21% and 29% over the past
decade. A single year is noisier: in 2024 the air kept 7.9 GtC, so about 32% came back (31% on
NOAA's measured 3.76 ppm rise); the budget's sink models said 46%, and the gap is its stated -1.7
GtC imbalance. Atmospheric CO2 averaged 425.6 ppm in 2025 (NOAA, measured). The oxygen loop is
closed by the biosphere: what people breathe out (about 2.2 Gt of CO2 a year) is this year's food
carbon.

**Water (SDG Extended Report 2026, AQUASTAT).** Withdrawals 4,022 km3 (965 cubic miles) in 2023,
nearly flat since 2015: agriculture 71.7%, industry 14.9%, municipal 13.3%. That is 1,343 L (355
gal) a person a day, 179 L (47 gal) of it municipal, against 43,000 km3 of renewable freshwater.
Water stress averages 18% and passes 75% in Northern Africa and Western, Southern and Central Asia.
Of 332 km3 of household wastewater, 185 km3 (56%) was safely treated in 2024. Two of these rest on
one source each and are marked so: the 9.4% (withdrawals over an undated AQUASTAT denominator) and
the 56% (no second source read).

**Food (FAO, UNEP).** Dietary energy supply 3,016 kcal a person a day in 2023, against a population
need of about 2,100 (the planning figure for whole populations: enn-1997, and current practice in
fsc-2023, "as per Sphere Standards"). 13.3% of the food's value is lost between harvest and retail
(2023; the index weights commodities by the value of production, not by mass); 1.05 billion t, 132
kg (291 lb) a person, almost a fifth of what reaches consumers, is wasted (2022).

**Land and soil (FAO).** Agricultural land 4,800 million ha (cropland 1,600, pasture 3,200), forest
4,050, other 4,150, in 2023. Cropland 0.19 ha (0.47 acre) a person, down from 0.24 in 2001. Topsoil
forms at up to 1,000 years per centimetre (0.4 in).

**Nutrients (USGS, 2026).** Phosphate rock mined 250 million t in 2025; reserves 73 billion t,
resources more than 300 billion; "There are no substitutes for phosphorus in agriculture." 47.8
million t of P2O5 (20.9 million t of phosphorus) went into fertilizer. 160 million t of nitrogen was
fixed as ammonia (from air, which is unlimited, with energy that is mostly fossil). Potash 49
million t, more than 120 years of reserves. People excrete about 4.65 kg (10.3 lb) of nitrogen and
0.61 kg (1.3 lb) of phosphorus a year each on a Finnish diet (viskari-2018, urine plus faeces), 0.44
kg of phosphorus at Cordell's world average of 1.2 g a day (cordell-2010): all of humanity's excreta
hold about a fifth (17 to 24%) of fertilizer phosphorus, and about 10% of it is returned, 1.4 to
2.4% of fertilizer phosphorus. The bigger loop is manure: around 2005, of 24.5 million t of
phosphorus put on land, 10.5 million t (43%) was recycled (manure 8, crop residues 2, waste and
sewage 0.5) and 14 million t mined (cordell-2010's mass balance, with her stated high uncertainty).

**Materials.** 106 billion t (117 billion short tons) a year, 39 kg (86 lb) a person a day (2020,
gro-2024); 6.9% recycled, down from 7.2% in 2018 (cgr-2025, still the latest per cgr-2026); 62
million t of e-waste, 22.3% formally recycled (2022); 18 of 60 metals recycled above half at end of
life (2011, with data from 2000 to 2005).

### What "as close to 100% as possible with today's technology" would take

Every technology below operates somewhere today; what limits it is building it, not inventing it.

| Loop | Today | What closes it with operating technology | What still cannot close |
|---|---|---|---|
| Air | about 50% of carbon returned over a decade (32% in 2024) | Electricity from wind, sun, rivers and nuclear for the 86% of traded energy that is fossil; electrified heat and transport | Process emissions (cement) and land-use change need their own fixes |
| Water | 56% treated | Treat and reuse all wastewater; keep each basin's withdrawal under its renewable flow | Closure is local, basin by basin |
| Food | about 70% reaches people | Halve loss and waste; return food waste and excreta to farmland | Soil lost to erosion does not come back on a human timescale |
| Nutrients | about 43% of land phosphorus recycled (2005), 1.4 to 2.4% of fertilizer P from people | Recover phosphorus from wastewater, sludge ash and urine (not sourced in this pass); ammonia from clean hydrogen | Phosphorus reaching the sea; about 290 years of reserves |
| Energy | 8.6% of traded supply from flows (18% of final consumption) | As Air | Equipment is still mined and manufactured |
| Materials | 6.9% recycled | Design for repair and recovery; collect e-waste; use less, since growth in use is what lowers the share | Elements recycled below 1% |
| Habitat | 0.19 ha a person | Keep soil in place and building | Land per person falls as population grows |

For N people on Earth the homestead model in section 4 is the bound: the bare tier fits on less
land than agriculture already uses; the comfort tier does not fit on Earth.

---

## 6. Scenario B: the game's mothership

### 6.1 The ship
The lore: about 500 km (311 mi) long, a cylinder about 100 km (62 mi) across with rotating habitat
rings, for up to 10 billion people (game-lore-humanity-one; mothership-superstructure.md reads it
as 500 by 100 km). The test takes the size as a parameter. Derived: 3.93 million km3 (942,000 cubic
miles) of volume, 479,000 m3 (16.9 million cu ft) per person, 275 times SP-413's 1,738 m3; a 50 km
radius spins at 1 g once every 7.5 minutes (0.13 rpm). If every person had the player's acre (55 by
89 m, 4,895 m2, 1.21 acres), the decks would total 40.1 million km2, about a tenth of the hull at 10
m between decks. **Volume is not what limits this ship. Heat, power and minerals are.**

### 6.2 What has been done, with people inside
| System | When | Result | What fell short |
|---|---|---|---|
| ISS water recovery | 2023 | 98% with the brine processor, 93 to 94% before (nasa-water-2023, boyce-2024) | 2% is dried brine, dumped |
| ISS oxygen from CO2 (Sabatier, ESA's ACLS) | today | about half (greenwood-2017, dominguez-2024, esa-acls) | the methane is vented with its hydrogen |
| BIOS-3, Krasnoyarsk | from 1972 | oxygen and water 100%; 80% of food at 30 m2 a person; 100% estimated at 56 m2 (salisbury-1997) | **minerals only about 20% retained**; faeces dried and stored; about 400 kW for three people |
| Biosphere 2, Arizona | 1991 to 1993 | 1.27 ha (3.1 acres), 200,000 m3, eight people (dempster-2008) | oxygen from 20.9% to 14.4% in 16 months, into soil microbes and concrete |
| Lunar Palace 365, Beijing | ended 2018 | 370 days, four people at a time, 98.2% "material closure" (beihang-2026) | the measure is not defined on the page read |
| MELiSSA, Barcelona | since 2009 | pilot plant with animals as its crew, aiming at a human crew by 2020 to 2025 (esa-melissa-2009) | whether that aim was met was not read |

### 6.3 What one person needs aboard
`mothership.ron`'s `per_person` carries each with its derivation:

- **Crops**: 50 m2 (540 sq ft) of lit canopy (NASA's 40 to 50, 35 to 40 with insects; BIOS-3's 56),
  65 m2 (700 sq ft) of floor with 25 to 50% for crew access (bvad-2022). Crops sized for food also
  make all the oxygen and take up all the CO2 (BIOS-3; johnson-wheeler-2021).
- **Light**: about 9 kW (3.5 to 16), at the 2.8 umol/J fixtures had demonstrated by 2020
  (kusuma-2020; the first pass used the 3.4 calculated limit and got 8). Three routes: BIOS-3's 8.5
  kW of light for full closure, 13.9 kW; BVAD's measured biomass per kWh, scaled from its 1.7 umol/J
  lamps to 2.8, 2.3 to 24 kW; first principles from photons per gram, 3.8 to 12.5 kW. A fourth,
  used to try to refute it (a modelled plant factory's 247 kWh per kg of dry lettuce,
  graamans-2018), gives 5.7 to 7.0 kW, inside the range. Still derived: no crewed system has
  measured a whole diet's electricity under modern LEDs.
- **Air and water processing**: about 0.5 kW, mostly the fans that condense the crops' 200 L (53
  gal) a day of transpiration back out of the air, at the game's own measured 27 Wh per litre.
- **The household**: about 0.4 kW (the solo home's 8.7 kWh a day without life support).
- **Industry and repair**: about 0.5 kW, **an estimate with no source** (phase 2 replaces it).
- **Total about 10.4 kW, 250 kWh a day.** For comparison, Earth's electricity is 0.45 kW a person.
- **Backup machines** for the air if the crops fail: 7 to 9 kWh a day while they run.
- **Water**: at the ISS's 98%, a comfortable household loses 1.7 L (0.45 gal) a day, 0.6 t a year:
  5 billion t a year for 8.2 billion people. A thousand-year ship must return its brine and solids
  through its biological loop and lose only what leaks.

### 6.4 Heat
Every watt the reactors make ends as heat the ship must radiate into space, and there are two
streams. **The habitat's**: the 10.4 kW a person uses, shed at about room temperature. A fin
radiating from both sides at 290 K (17 C, 62 F) with emissivity 0.9 sheds 722 W per m2 (67 W per sq
ft): 14.4 m2 (155 sq ft) per person. **The reactors' own waste heat**, the larger stream. The
game's reactor is the KLT-40S class (data/ship_power.ron), whose datasheet gives 2 x 150 MW of heat
for 2 x 35 MW of electricity, 23.3%, from a core whose water leaves at 316 C (589 K) and a
water-cooled condenser (klt40s-iaea). So each person's 10.4 kW comes with 34 kW of waste heat, shed
at its condenser near 300 K (827 W per m2): another 41 m2. **Together about 56 m2 (600 sq ft) a
person, 457,000 km2 (176,000 sq mi) for 8.2 billion, 2.9 times the hull's whole side.** Fins as
large as the side serve about 2.8 billion people; at an unsourced 33% plant, 40 m2 a person and 3.9
billion.

The first pass had this badly wrong, and the error is worth recording: it said the reactor's waste
heat would be "rejected hotter, at 600 K ... 1.4 m2 more per person". No light-water reactor can do
that: Carnot allows no work at all from a 589 K core rejecting at 600 K, and at the KLT-40S's 23.3%
the condenser can be no hotter than about 450 K. Raising the condenser temperature toward that
bound trades efficiency for radiator area, and finding the best trade is a design problem this plan
does not solve. The game does not model any of this yet (ship-life-support.md section 5, Climate);
it is the next rung of that work.

### 6.5 Fuel for a thousand years
- **Fission** (operating today): about 140,000 t of heavy metal fissioned a year for 85 TW at the
  KLT-40S's 23.3% (99,000 at an unsourced 33%), 17 kg (38 lb) a person, about 140 million t over the
  voyage, which needs breeder reactors to use most of the uranium or thorium carried (not sourced).
- **Fusion, deuterium-tritium** (research): 17.59 MeV a fusion, plus 4.78 MeV more when the lithium
  blanket breeds its tritium (Li-6 + n -> T + He-4), 22.37 MeV in all, from NIST masses. The ship
  carries deuterium and lithium-6, 269 TJ a kg, so per joule it carries about 0.31 of fission's fuel
  mass. At an ASSUMED 33 to 40% plant efficiency (no fusion plant has demonstrated one): 25,000 to
  30,300 t carried a year (15,700 to 19,000 t of D-T burned), 25 to 30 million t over the voyage.
  The deuterium comes from water: seawater sits near the isotope standard VSMOW, 155.74 parts per
  million of hydrogen atoms (ciaaw-hydrogen), about 34.8 g a tonne. (The first pass left out the
  blanket, giving 0.243, and "corrected" the deuterium to 26 g with a value for natural materials in
  general; both are fixed.)

### 6.6 Nutrients
The farm takes up phosphorus in proportion to what it grows: 0.9 to 1.5 kg of dry biomass a day
(the same biomass the lighting figure rests on) at 0.25% phosphorus is 0.82 to 1.37 kg (1.8 to 3.0
lb) a person a year. People excrete 0.44 to 0.61 kg, nearly all they eat, and with about 0.63 of a
plant's phosphorus in its edible part that independent route gives 0.70 to 0.97: the two agree.
Kept at 20% (BIOS-3's average over all minerals; applying it to phosphorus is an assumption, the
paper gives none for phosphorus), about 0.9 kg a person is lost each year: 7.2 million t for 8.2
billion people (5.4 to 9.0), a quarter to two-fifths of the 20.9 million t of phosphorus in Earth's
fertilizer. At 99%, 90,000 t a year; at 99.9%, about 9,000 t a year and 9 million t for the voyage.
There is no mine between the stars. (The first pass used data/home_outline.json's 2.6 kg, a farm
grown about twice as large as its resident eats to trade grain, and got 17 million t a year.)

### 6.7 Habitat: shielding, spin, and what is open
Shielding over the whole hull at SP-413's 4.5 t/m2 is 780 billion t, 95 t (105 short tons) a person;
shielding only the inhabited rings needs less. Rotation is gentle at 50 km. **Open, and not sourced
in this pass**: whether any material can carry a 50 km-radius ring at 1 g with its decks, soil, air
and people (the hoop stress grows with the radius), the leak rate of a 500 km hull, and a modern
shielding study to replace SP-413's 1975 one.

---

## 7. Research options, with readiness, and what matters most

`technologies.ron` carries each option: what it is, its NASA Technology Readiness Level **for a
ship's life support** (which can be lower than on Earth), the evidence, its measured performance
with quotations, and exactly which figures it changes, so a test or a player can switch it on.
Readiness (0, 0) means not assessed: named, and refused by the tests until sourced.

| Option | Loop | Readiness | Evidence | What switching it on changes |
|---|---|---|---|---|
| ISS water recovery | Water | 8 to 9 | 98% (2023) | water make-up 1.7 L a day |
| Sabatier and ACLS | Air | 9 | about half of the oxygen | oxygen water draw |
| Electrolysis, sorbent bed, contaminant control | Air | 9 | on the ISS | backup 7 to 9 kWh a day |
| Crops under electric light, whole diet | Food | 5 to 6 | BIOS-3, Lunar Palace 365 (ground); fixtures 2.5 to 3.0 umol/J demonstrated | the 9 kW |
| Fission | Energy | 6 to 9 | a floating plant supplies Pevek since 2020, 23.3% efficient; 1,000 years of breeding never done | fuel about 140,000 t a year; 41 m2 a person of radiator for its waste heat |
| Mass shielding | Habitat | 9 | the Earth's air | 4.5 t/m2 |
| Phosphorus recovery on Earth | Nutrients | not assessed | not sourced | none until sourced |
| **Fusion (D-T)** | Energy | **3 to 4** | NIF 8.6 MJ from 2.08 MJ on target (2025); ITER D-T planned 2039 | fuel carried about 0.31 of fission's per joule; habitat power unchanged; plant waste heat unproven |
| Methane pyrolysis | Air | 4 to 5 | ground prototypes, theoretical more than 86% oxygen recovery | oxygen water draw 0.18 kg a day |
| Electrochemical oxygen recovery | Air | 3 | laboratory, aiming at 75% | none yet |
| **Microbial protein from hydrogen** | Food | 4 (8 on Earth) | a commercial plant; O2/CO2 2.7; 24 kWh/kg in a 2017 lab | food power about 6.5 kW if it is 30% of calories (a game choice) |
| Food grown on electrolytic acetate | Food | 3 to 4 | laboratory; about fourfold over photosynthesis in sunlight | none yet (no electricity per kg read) |
| LEDs at their calculated limit | Food | 6 to 8 | 3.4 and 4.1 umol/J calculated limits | food power 7.4 kW (3.4) or 6.1 kW (4.1) |
| **Full mineral recycling (MELiSSA-style)** | Nutrients | 4 to 5 | pilot plant with animals | phosphorus about 1 g a person a year |
| Active (magnetic) shielding | Habitat | 2 to 3 | rejected by SP-413 in 1977 | none |
| Medicine on demand; capturing matter on the way | Materials | not assessed | not sourced | none until sourced |

### 7.3 What matters most, and why fusion is not first

Rank the options by how much of the ship they change. **Food per kWh comes first** because lighting
is 87% of the electricity, and the electricity sets the reactor size, the fuel and, through the
reactors' waste heat, most of the radiators. **Heat rejection and conversion efficiency come
second**: at the game's 23.3% reactor rejecting at 300 K, the radiators for the reactors' own waste
heat are three times those for the habitat, and together they limit the hull to about 2.8 billion
people. **Mineral recovery comes third** because at today's only measured figure the ship would lose
a quarter to two-fifths of Earth's fertilizer phosphorus every year, a gap of about 800 times in
losses between today and what is needed. **Material recovery** decides which machines can still be
rebuilt after a few centuries. **Fusion** comes fifth for the ship as a whole: it is a better fuel
(about 0.31 of fission's fuel mass per joule, deuterium and lithium; no long-lived actinides), and it
leaves the habitat's power and heat where they are. Whether it shrinks the reactors' radiators
depends on a fusion plant's efficiency and the power it spends on itself, neither of which any
plant has demonstrated. For the operator's thousand-year frame it matters most for what is carried:
25 to 30 million t of deuterium and lithium instead of about 140 million t fissioned, and no
centuries of spent fuel aboard.

---

## 8. The test itself (phase 2)

### 8.1 The rule: the game's own systems, never a separate model
The test runs `src/systems/life_support.rs`, `farming/` (humidity, soil, crops),
`grow_machines.rs`, `electrical.rs`, `ship_power.rs`, the plumbing system and the machines' meters
(`src/machines.rs`), on the shipped homes, the way
`the_shipped_homes_hold_their_air_and_return_their_water` already does (life_support_tests.rs
`shipped_home`). The larger scales cannot run every machine, so they run an **aggregate tier**: a
ledger per zone with a population count (the `population: u64` per zone that
mothership-superstructure.md concluded was the achievable model), whose per-person coefficients
are **measured from the home runs**, never typed in. The calibration case
`aggregate_matches_home_at_n1` holds the aggregate to the home within 2% on every loop, which is
what keeps it from becoming a drifting twin (headless-sim-runner.md, risk 1). Where the game and
these data files disagree, that is a finding to record, not a tolerance to widen.

### 8.2 The cases
`test_cases.ron` holds the full list with every expected value and tolerance; in short:

| Case | Tier | Needs first | What it proves |
|---|---|---|---|
| every_closed_loop_figure_is_sourced | Unit | the loader | no number without a source, a read date and a check |
| person_breathes_by_the_current_nasa_baseline | Unit | the loader | the game's breathing matches BVAD 2022 within 1% per MJ |
| closure_is_never_one_number_by_mass | Unit | the loader | the 99.1% trap is never printed alone |
| same_seed_same_bytes | Unit | sim seeds | closure numbers are reproducible |
| solo_home_closes_what_it_closes | HomeDay | nothing: writable today | the solo home's measured closure per loop |
| family_home_closes_what_it_closes | HomeDay | nothing | the same at three residents |
| one_to_three_residents_per_person | HomeDay | nothing | the first measured non-linear terms |
| time_to_harm_when_one_thing_fails | HomeDay | nothing (coarse steps faster) | the failure margins table, from the game |
| solo_home_year | HomeYear | runner increment 2 | stocks over a year; the phosphorus ledger closes to 0.1% |
| dt_ladder_closure_is_step_invariant | HomeYear | runner increment 2 | the safe step for every closure case |
| a_hidden_sink_is_found | HomeYear | runner increment 2 | a Biosphere 2-style leak is caught within a year |
| aggregate_matches_home_at_n1 | Calibration | the aggregate tier | the big scales rest on measured coefficients |
| earth_today_reproduces_the_measured_world | Calibration | the aggregate tier | the aggregate's arithmetic on sourced inputs |
| population_ladder | Aggregate | the aggregate tier | 1 to 8.2 billion, Earth and ship |
| mothership_with_todays_technology | Aggregate | the aggregate tier | 10.4 kW, 85 TW, 56 m2, 2.8 billion, the phosphorus make-up |
| each_research_option_changes_only_what_it_says | Aggregate | the aggregate tier | each switch changes its figures and nothing else |
| simplified_mode_agrees_with_full | Calibration | the aggregate tier | the two modes never give different answers |
| a_thousand_years | Millennium | all of the above | the drift processes and the first stock to run out |

**The independent-route cases** (added in the second review). The critic's main point was that not
one case above could have caught the errors it found: the radiator case re-derived the same
arithmetic without the reactor term, and the others hard-coded the questioned values. A check whose
evidence is its own setup cannot fail (the project's standing lesson). So each load-bearing figure
now also has a case that checks it by a route sharing none of its inputs, each named with the
first-pass error it would have caught, and each must be seen red on that error before it is
trusted:

| Case | Independent route | Would have caught |
|---|---|---|
| bvad_table_reading_balances | conservation of mass over the source's own table | a value read against the wrong label |
| heat_engines_obey_carnot | the second law, from the reactor datasheet's temperatures | "waste heat rejected at 600 K" from a 589 K core |
| radiators_carry_all_the_reactor_heat | an energy ledger from the reactor's thermal power, not the electricity | 13 m2 a person, which radiated 9.4 of about 40 kW |
| the_reactor_class_matches_ship_power | a second data file (data/ship_power.ron) | an unsourced 33 to 40% efficiency |
| phosphorus_uptake_matches_what_people_excrete | the person's excreta over the plant's edible share | a farm sized for trade (2.6 kg) |
| carbon_uptake_is_measured_by_the_air | NOAA's measured rise, not the sink models | 46% for 2024 (measured: 31 to 32%) |
| fusion_energy_counts_both_reactions | mass defects and the isotope standard | no blanket (0.243), 26 g of deuterium a tonne |
| homestead_land_is_the_sum_of_its_parts | the parts | the outline's 0.34 ha (the parts give 0.36) |
| excreta_is_urine_and_faeces | the table's own rows and a second source | greywater counted as excreta |
| today_uses_only_demonstrated_performance | the kind of each number (measured, limit, derived) | a calculated LED limit used as today's |
| transpiration_heat_fits_inside_the_light | the Water loop's transpiration against the Energy loop's light | a lighting figure too low for its canopy |
| every_scope_is_printed | the report's own output | 8.6% taught without "of commercially traded supply" |
| every_quote_is_in_its_source | the source's own text, fetched (on demand, not CI) | a quotation that is a summary's words |

Each case lists its inputs, expected outputs with tolerances, its pass line, its compute and memory
budget, the systems it runs, and what must exist first. Several expected values are marked
**Report**: printed and recorded with no pass line, because nobody yet knows what right looks like
(the oxygen the solo home's crops make back, the food a year really grows, a fusion plant's
radiators), and pretending otherwise would be a check that cannot fail.

### 8.3 Simulated time, compute and memory
| Tier | Simulated time | Step | Budget each | Where it runs |
|---|---|---|---|---|
| Unit | none or a day | none or 6 s | under 0.2 s, 20 MB | `just verify` |
| HomeDay | two game days | 6 s (today's harness) | 3 to 10 s, 300 to 400 MB | `just verify` |
| HomeYear | a game year | the safe step, 600 s expected | 120 s, 500 MB | nightly |
| Calibration and Aggregate | a year at each population | a day | under 2 s, 100 MB | `just verify` |
| Millennium | 1,000 years, jumping a year, re-measuring a home-year every century | a year | 15 minutes, 1 GB | nightly; a 10 s frozen-rate variant in CI |

The CI cases together stay under 60 s of one core and 600 MB. The nightly cases run at most four at a
time, inside the share of the PC the operator keeps (scripts/pc-share-guard.ps1). No GPU, no window,
no audio, no network, no writes outside a scratch storage root. At the aggregate tier the cost is a
few operations per zone per loop per step: 100,000 zones at a day a step is, by estimate, about 1 ms
a step, so a simulated year is about 0.4 s, independent of how many billions live in the zones.
Phase 2 measures it and replaces the estimate.

### 8.4 What the headless runner must provide
From headless-sim-runner.md, in the order this test needs it:

1. **Increment 2**: `--sim soak` with a script, `sim_seed`, game-day JSON lines of every loop's
   ledger and every stock, and the dt ladder that sets each home's safe step, with
   `data/sim/step_tolerances.ron`.
2. **The jump** (`catch_up_world` with a moved `now`) for the Millennium tier, extended to apply
   drift.ron's processes to the stocks at each jump; and the **away-versus-live** comparison, so a
   jumped year is checked against a lived one.
3. **Parallel runs** (four) and the **determinism** test (same script, same bytes).
4. **A switch per research option** read from technologies.ron (a DataStore slot), so the same run
   can be repeated with one option on.
5. **Rate classes** (increment 7): FarmingSystem at a slower rate inside its tick, which makes a
   home-year affordable.

### 8.5 How the data loads
One module, `src/systems/closed_loop/` (data.rs, report.rs, aggregate.rs): serde structs mirroring
`schemas/closed_loop.toml` field for field, `ron::from_str` on each file the way
`data/life_support.ron` is loaded, a loader test that refuses any unknown source id, any figure
without a read date or a check, and any research option without a readiness level, and the eight
files added to `src/embedded_data.rs` so a release carries them.

---

## 9. The 1,000-year view

A thousand years cannot be stepped. The Millennium tier jumps a year at a time and applies
`drift.ron`'s processes to the stocks, and re-measures a home-year every century of game time so the
rates come from the real systems. The processes, and what they give:

| Process | Model | Over 1,000 years |
|---|---|---|
| Material recovered each product life | stock x r^(t / life) | r = 0.95, 20-year lives: 7.7% left; 0.99: 60.5%; 0.999: 95.1% |
| Human genetic diversity | heterozygosity x (1 - 1 / (2 Ne))^(generations) | Ne 150: 87.5 to 89.5% kept; Ne 23,400: 99.9% |
| Crop varieties in the seed bank | not assessed | regeneration interval and planting per variety to be sourced |
| Topsoil | formed at up to 1 cm per 1,000 years | loss rate to be sourced |
| Earth's phosphate reserves | 73 billion t at 250 million t a year | 292 years; resources about 1,200; the pessimistic reading 50 to 100 |
| The ship's phosphorus | lost at (1 - retention) of 0.82 to 1.37 kg a person a year | about 9 million t carried at 99.9% (6.7 to 11.2), 90 million t at 99% |
| Reactor fuel | at 85 TW | about 140 million t fissioned at 23.3%, or 25 to 30 million t of deuterium and lithium-6 at an assumed fusion efficiency |
| Air through the hull | 0.02 kg of air a day per module | a leak law for a 500 km hull not found |
| Hidden sinks (Biosphere 2) | stock against ledger, every year | any gap reported, never absorbed |
| Equipment wear | failure rates, spares, the industry that makes them | the first year a machine cannot be replaced |
| Skills | a minimum population per industry | which loops reopen at each population |
| Radiation dose | 5 mSv a year behind the design shield | about 400 mSv a life; one unshielded year is past a career limit |

Two disagreements are carried rather than settled. **Genetic diversity**: the simple heterozygosity
formula keeps about 90% for 150 people over thirty generations, while a popular account of Smith's
work says such a crew loses 80% of its diversity (rare variants go far faster than heterozygosity);
the test reports heterozygosity and names the other measure as missing. **Phosphate**: 290 years of
reserves by USGS today, against 50 to 100 years of affordable rock in Cordell's 2009 reading, most of
the difference being one 2010 revision of Morocco's reserves from 5,700 to 51,000 million t
(edixhoven-2014, who question it).

---

## 10. Two modes

The house rule: every deep system ships full realism and a simplified mode.

- **Full realism**: every loop from the game's own systems; stocks, buffers and failures; drift;
  research options switched by readiness; the Realistic power mode (the home runs on what it makes).
- **Simplified**: per-person coefficients from the calibration case, no failures, no drift, a green,
  amber or red verdict per loop with its closure fraction. Station-supplied power (the reactor
  covers what the home does not make, metered) is the simplified mode's default, as it is the game's.

The rule between them (`simplified_mode_agrees_with_full`): the simplified mode may be coarser, never
different in its answer. A loop that does not close in full realism does not close in the simplified
mode either.

---

## 11. The refutation passes

Every load-bearing figure was put to a second source, twice: once by this plan's author, then by an
independent critic whose findings were each re-read before they were applied. The per-figure
results are in each figure's `check` (80 figures in `data/closed_loop/`: 26 survived a second
source, 24 remain uncertain, 22 are derived and were recomputed a second way, 8 are marked
corrected, and further corrections were made inside budgets and derivations, listed below).

### 11.1 The first pass

**Survived a second source** (and still standing after the second review).
- NASA's breathing figures: BVAD 2022 and Hanford 2004, 18 years and two different crewmembers
  apart, agree within 0.9% per MJ of food burned; BVAD's mass balance closes to the gram.
- ISS water recovery 98%: NASA's release and the brine processor's own 2024 paper (which shares an
  author, so it corroborates rather than confirms).
- ISS oxygen recovery about half: four NASA and ESA documents.
- Crop area per person: BIOS-3's table against NASA's review.
- Earth's energy supply, fossil share and per-capita energy: the review's own totals against its per
  capita figure and press coverage; Earth's population against the review's implied 8.23 billion.
- Water withdrawals: the three sectors sum to the stated total within 0.05%.
- The deep-space dose: 1.8 mSv a day (SwRI) against 1.84 +/- 0.3 (a DLR record seen in a search).
- Microbial protein electricity, to an order of magnitude: the derived 31 to 34 kWh per kg against a
  2017 laboratory figure of 24.

**Corrected in the first pass.**
- FAO food loss: this pass began from a remembered 13.2%; no FAO text read says it; 13.3% (2023) is
  the portal's.
- BIOS-3's closure table: the scan's layout garbled it; read from the raw text order, with two OCR
  errors ("SO" for 80, ".10" for 30) fixed by position.
- The excreta-phosphorus ratio: a unit slip printed 294 (then 0.29; the second review corrected it
  again, below).
- A hull-flush radiator radiates from one side only.
- **Dropped**: a search result's claim that a methane pyrolysis prototype recovered "up to 75%" of
  the lost hydrogen was not in the paper's text, and is not used.

### 11.2 The second review (an independent critic, each finding re-read here)

**Refuted, and fixed.**
- **Reactor waste heat "rejected hotter, at 600 K, 1.4 m2 more per person".** Re-read in the
  KLT-40S status report (klt40s-iaea): 2 x 150 MW of heat for 2 x 35 MW of electricity gross, 23.3%,
  core water out at 316 C (589 K), a water-cooled condenser. No such reactor can reject at 600 K
  (Carnot). The waste heat is about 34 kW a person, rejected near 300 K: 41 m2 more radiator, 56 in
  all, 2.9 times the hull's side; fins of the side's area serve 2.8 billion people, not 12 billion.
  The fission fuel at 23.3% rises to about 140,000 t a year. The heat-limit conclusion holds, much
  more strongly.
- **Phosphorus make-up of 17 million t a year.** Re-read in data/home_outline.json: its 2.6 kg is a
  farm grown about twice what its resident eats, to trade grain. The ship's own biomass gives 0.82
  to 1.37 kg of uptake, 5.4 to 9.0 million t a year lost at 20%, 6.7 to 11.2 million t carried for
  the voyage at 99.9%. And BIOS-3's 20% (re-read) is an average over all minerals; for phosphorus the
  paper says only that input and output "were nearly balanced", so applying 20% to phosphorus is
  stated as an assumption.

**Corrected.**
- Carbon uptake "46% in 2024" was the sum of the sink models; what the air shows (emissions less the
  measured rise) is 32% from the budget's own 7.9 GtC, 31% on NOAA's 3.76 ppm (re-read in NOAA's
  file). The decade's 50% stands; 425.7 ppm stands against NOAA's measured 425.62.
- Fusion fuel: the blanket's Li-6 + n -> T + He-4 adds 4.78 MeV (recomputed from NIST masses, Li-6
  re-read), 22.37 MeV a fusion; the ship carries deuterium and lithium-6, 0.31 of fission's mass per
  joule, not 0.243. The deuterium "correction" to 26 g a tonne used NIST's value for natural materials
  in general; seawater sits near VSMOW (re-read at CIAAW): 34.8 g, as the first draft had it.
- Homestead land: the outline's parts sum to 0.28 to 0.36 ha, not 0.34 (the outline is fixed too),
  0.24 to 0.28 without the woodlot; "a land question before a technology question" was dropped, the
  outline itself saying the woodlot disappears with a different heat source.
- Excreta: Viskari's Table 1 is "excreta and greywater"; excreta proper are 4.65 kg of nitrogen and
  0.61 kg of phosphorus. Cordell's own thesis (re-read, Appendix A) gives 1.2 g of phosphorus a day
  and sewage recycling of 0.3 million t, about 10%: excreta hold 17 to 24% of fertilizer phosphorus
  and 1.4 to 2.4% of it comes back. Reading the same table added the whole-system figure: about 43%
  of the phosphorus put on land was recycled around 2005, mostly as manure.
- The 8.6% energy share now carries its scope (commercially traded supply only; re-read) beside SDG
  7.2.1's 18.0% of final consumption, 13.4% without traditional biomass (re-read).
- LEDs: real fixtures had reached 2.5 to 2.8 (white plus red) and 3.0 (blue plus red) by 2020
  (re-read in Kusuma). **This review's own addition**: the first pass's "today" lighting used the 3.4
  calculated limit; today now means demonstrated, 2.8, so the lighting goes from 8 to 9 kW and the
  whole budget from 9.4 to 10.4 kW (85 TW). The limits are the research option, saving 18 to 32%.
- Marin and Beluffi's abstract claims "a healthy population virtually indefinitely" (re-read), so the
  "survive at all against stay healthy" gloss is gone: the two sources differ in their models.
- FAO's loss index weights commodities by value of production (re-read in its metadata), not mass.
- 6.9% circularity: confirmed in the 2025 report itself (re-read: "from 7.2% in 2018 to 6.9% as of
  the latest available data (2021)"), still the latest per the 2026 report: now Survived.
- NASA-STD-3001 Volume 1 Revision C (approved 2023-09-15) keeps 600 mSv (re-read); Smith 2014 was read
  as the author's accepted manuscript: Primary.

**Disputed, or refined.**
- The critic's "rejected at 300 K" is the datasheet's seawater condenser, the conservative case, not
  a law: a space plant may run its condenser hotter, up to Carnot's bound (about 450 K at 23.3%), at a
  cost in efficiency. The data carry 300 K and say the trade is unsolved.
- The critic's fission fuel (about 127,000 t a year) is right for 9.4 kW; with the demonstrated-LED
  rebase above it is 140,500.
- The critic's carbon "measured uptake 31%" is 31 to 32% depending on whose growth figure is used;
  both are carried.
- The KLT-40S table labels 23.3% "net" although its net output (2 x 30 MW) gives 20%; the test uses
  the gross 23.3% and carries 20% as the low end.
- Rev C's date: approved 15 September 2023, uploaded in November.

**Still unproven, and said so in the files**: the 9.4% of renewable freshwater (one undated
denominator); the 56% of wastewater treated (no second source); the 9 kW lighting point (every
route is derived); a fusion plant's efficiency and recirculating power, so its waste heat; a
phosphorus-specific retention for any crewed closed system; UNEP's exact 19%; the 2,100 kcal figure,
now confirmed as current practice by a 2023 humanitarian guideline (fsc-2023) but not read in the
WHO, UNHCR, IFRC and WFP edition itself.

**Still uncertain from the first pass**: Lunar Palace 365's undefined 98.2%; the phosphate reserve
range (50 to 292 years); SP-413's 1975 shielding; every population threshold above a household
except the genetic ones and the chip supply chain.

**Still unknown** (phase 2 must source these before a test asserts them): recovery rates and
readiness of phosphorus recovery from wastewater; a leak law for a large hull; whether any material
carries a 50 km ring at 1 g; the best condenser temperature for a ship's reactors; electricity per
kg of food grown on acetate; how much of a diet microbial protein can safely be; micronutrients in a
grown diet; seed-bank regeneration; topsoil erosion rates; per-year spares mass for life support;
the industry share of a ship's power; the population a clinic, a foundry or a fab needs; whether
MELiSSA has supported a human crew.

---

## 12. What does not balance

1. **FIXED: the solo home's Nutrients loop said it closed** (`data/machines/home_solo.ron`,
   `closes: true`, "the same way the 3-person home's does"). The family home's loop has said it does
   not since the 2026-09-26 garden model, the outline's audit finds compost and excreta return about
   53% of the nitrogen, 75% of the phosphorus and 93% of the potassium, and cannot_close.ron's footer
   says nutrients only nearly close. Set to `closes: false` with a note saying why. The Home page's
   "N/M loops close" line for the solo home drops by one. No test asserted the old value.
2. **FIXED: data/home_outline.json summed the bare-tier land to "0.28 to 0.34 ha"**; its own parts
   (greenhouse 200 m2, arable 1,700 to 2,100, woodlot 400 to 800, house and yard 500) give 0.28 to
   0.36. The shelter line now shows the sum and says what it read. `homestead_land_is_the_sum_of_its_parts`
   would catch it again.
3. **A single closure-by-mass number hides an open food loop** (section 1). Not a bug in the game,
   which reports loops separately already; a rule for the test's reports.
4. **The game's people breathe on Hanford 2004's figures, not BVAD 2022's.** Per MJ of food they
   agree within 1%, so nothing changes; the unit case holds them together so a future edit to either
   is caught.
5. **The mothership's dominant loads are unmodelled**: the solo home's garden is sun-lit through
   glass in the game, but between the stars there is no sun, so every ship farm is lit by
   electricity, and that light is 87% of the ship's electricity; and the reactor class the game
   carries (data/ship_power.ron) wastes three watts of heat for every one it delivers. The game models
   neither crop lighting at ship scale nor heat rejection (ship-life-support.md section 5, Climate).
6. **The lore's ship cannot shed the heat of 8.2 billion people** with radiators the size of its
   hull's side and today's reactor class (about 2.8 billion; section 6.4). This is a finding for the
   ship's design, not the test: radiators beyond the hull, hotter or more efficient reactors, or less
   electricity per person.
7. **Already recorded and still true**: the solo home's Energy loop does not close in either mode
   (4.45 kWh a day made, 8.7 and 31.8 used), and its Food loop grows about 81% (home_solo.ron).

`data/self_sufficiency/cannot_close.ron` is **unchanged**: nothing in this pass changes what one home
can close. At the scale of all humanity aboard, electronics, medicine, metallurgy and equipment
renewal can all exist, because there are enough people to staff every industry; what cannot be
made between the stars is new matter, which is the raw chemistry entry's point made larger.

---

## 13. Phase-2 work list

In order; each is a rung of the whole plan above, not a throwaway.

1. **The loader** (`src/systems/closed_loop/data.rs`): serde structs per `schemas/closed_loop.toml`;
   the eight files into `src/embedded_data.rs`; `every_closed_loop_figure_is_sourced`, seen red on a
   deliberate break. Relay-gated `#[cfg(feature = "native")]` only if it touches native-only code (it
   should not).
2. **The report** (`report.rs`): the per-loop table, dual units, never one number, every fraction
   with its scope; `closure_is_never_one_number_by_mass`, `every_scope_is_printed`,
   `person_breathes_by_the_current_nasa_baseline`, and the independent-route unit cases
   (`bvad_table_reading_balances`, `heat_engines_obey_carnot`, `the_reactor_class_matches_ship_power`,
   `fusion_energy_counts_both_reactions`, `excreta_is_urine_and_faeces`,
   `today_uses_only_demonstrated_performance`, which needs a `kind` field on performance entries,
   and `homestead_land_is_the_sum_of_its_parts`, which needs the outline's parts as numbers). Each is
   seen red on the first-pass error it names before it is trusted.
3. **The home-day cases, today, on the existing harness**: `solo_home_closes_what_it_closes`,
   `family_home_closes_what_it_closes`, `one_to_three_residents_per_person`,
   `time_to_harm_when_one_thing_fails`. These need no runner and give the first measured closure per
   loop of the real homes.
4. **The aggregate tier** (`aggregate.rs`): a per-zone ledger with `population: u64`, per-person
   coefficients measured by step 3; `aggregate_matches_home_at_n1`,
   `earth_today_reproduces_the_measured_world`, `population_ladder`,
   `mothership_with_todays_technology`, `each_research_option_changes_only_what_it_says`,
   `simplified_mode_agrees_with_full`, and the independent-route aggregate cases
   (`radiators_carry_all_the_reactor_heat`, `phosphorus_uptake_matches_what_people_excrete`,
   `carbon_uptake_is_measured_by_the_air`, `transpiration_heat_fits_inside_the_light`).
4b. **The quote checker** (`every_quote_is_in_its_source`): a node script that fetches each source
   and searches its own text for each quotation; run on demand and before any release of the data,
   never in CI.
5. **With runner increment 2**: `same_seed_same_bytes`, `dt_ladder_closure_is_step_invariant`,
   `solo_home_year`, `a_hidden_sink_is_found`.
6. **The millennium tier**: drift.ron's processes applied at each jump, a per-material stock file,
   industry thresholds; `a_thousand_years` nightly with its frozen-rate CI variant.
7. **Sourcing debts** (section 11's "still unknown"), each before the case that would assert it.
8. **Game systems this plan asks for** (requests, not built here): crop lighting at ship scale as a
   power load; the Climate loop (heat to radiators) that ship-life-support.md already names as the
   next rung; a hull leak law; a phosphorus ledger across soil, compost and stores (the soil model
   exists, `farming/soil.rs`); a Home page row for each loop's closure fraction from the report.

---

## 14. Files

| File | What it carries |
|---|---|
| `docs/design/closed-loop-test.md` | this plan |
| `schemas/closed_loop.toml` | the shape of every file below, and how phase 2 loads it |
| `data/closed_loop/sources.ron` | 64 sources, each with its own date, the date read, how it was read |
| `data/closed_loop/person.ron` | one person's day, two tiers, the radiation and space a body needs |
| `data/closed_loop/earth_today.ron` | Scenario A: the measured world and each loop's closure |
| `data/closed_loop/mothership.ron` | Scenario B: the ship, what has been done, what one person needs |
| `data/closed_loop/technologies.ron` | options today and in research, with readiness and what each changes |
| `data/closed_loop/population_steps.ron` | one person to humanity, and what does not scale |
| `data/closed_loop/drift.ron` | the 1,000-year processes |
| `data/closed_loop/test_cases.ron` | the phase-2 cases, budgets and needs, including the independent-route cases |
| `data/machines/home_solo.ron` | the Nutrients loop corrected to `closes: false` |
| `data/home_outline.json` | the bare-tier land corrected to 0.28 to 0.36 ha, the sum of its parts |
