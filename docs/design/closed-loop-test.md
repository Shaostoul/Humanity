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
added on top.** Of what humanity emits, about half the carbon comes back to land and ocean in a
year; 8.6% of its energy comes from flows that refill; 6.9% of the materials it uses are recycled;
its phosphorus is mined, with no substitute. Water closes for the planet but not where people
live. Food is regrown every season but about 30% of it is lost or wasted.

| Loop | What the fraction is a fraction of | Earth today | Data year |
|---|---|---|---|
| Air (carbon) | Human CO2 taken back up by land and ocean | **about 50%** (46% in 2024) | 2015-2024 |
| Water | Household wastewater safely treated | **56%**; withdrawals are 9.4% of renewable freshwater | 2024, 2023 |
| Food | Food grown that is not lost or wasted | **about 70%** | 2022-2023 |
| Nutrients | Fertilizer phosphorus that comes back from people | **about 1.5 to 3%** (uncertain) | various |
| Energy | Energy supply from sustained flows | **8.6%** (86% fossil) | 2025 |
| Materials | Materials entering the economy that are recycled | **6.9%** | 2021 |
| Habitat | Cropland per person | **0.19 ha (0.47 acre)**, falling | 2023 |

**The mothership closes its air, water and food with technology that operates today, at a
price set almost entirely by light.** Every person needs about 50 m2 (540 sq ft) of crops lit day
and night, about 8 kW of electricity for those lights, and about 9.4 kW in all. For 8.2 billion
people that is 77 TW: twenty-one times all the electricity Earth makes, four times its whole energy
supply, and all of it heat the ship has to radiate into space (13 m2, 140 sq ft, of radiator fin
each at room temperature). The loops that decide whether the ship survives a thousand years are
not air or water: they are **nutrients** (the one crewed closed system whose mineral retention this
pass could read, BIOS-3, kept about 20%, by its own design; the ship needs 99.9%) and **materials** (nothing new arrives
between the stars).

| Per person aboard, today's technology | Per person | All 8.2 billion |
|---|---|---|
| Lit crop canopy | 50 m2 (540 sq ft), 35 to 56 | 410,000 km2 (158,000 sq mi) |
| Growing floor with crew access | 65 m2 (700 sq ft), 44 to 84 | 533,000 km2 (206,000 sq mi) |
| Electricity, all uses | **9.4 kW**, 225 kWh a day (3.8 to 18) | 77 TW |
| ... of which lighting the crops | 8 kW (3 to 15) | 66 TW |
| Radiator at 290 K (fin, both sides) | 13 m2 (140 sq ft); 26 m2 laid flat on the hull | 107,000 km2, two-thirds of the hull's side |
| Oxygen | 0.62 to 0.76 kg (1.4 to 1.7 lb) a day, made by the crops | |
| Phosphorus lost at 20% / 99% / 99.9% retention | 2.1 kg / 26 g / 2.6 g a year | 17 million t / 213,000 t / 21,000 t a year |
| Reactor fuel, fission / fusion (D-T) | 11 kg / 2.7 kg a year | 74,000 to 90,000 t / 18,000 to 22,000 t a year |
| Shield over the whole hull at 4.5 t/m2 | 95 t (105 short tons) | 780 billion t |

**The research options that matter most**, in order of what they change (section 7):

1. **Food per kilowatt-hour.** Lighting is 85% of the ship's power, its heat and its radiators.
   LEDs are near the limits of today's LED technology (3.4 to 4.1 umol/J; going from 2.9 to 4.1
   saves 29%). The next
   step is food that skips photosynthesis: protein from hydrogen-eating bacteria (a commercial plant
   on Earth, readiness 8 there and 4 aboard; a derived 31 to 34 kWh per kg, against a 2017
   laboratory figure of 24) and food grown on electrolytic acetate (laboratory, readiness 3 to 4).
2. **Mineral recovery.** At BIOS-3's measured 20% a ship of 8.2 billion would need about as much
   phosphorus every year as all of Earth's fertilizer. At 99.9% it carries 21 million t for the
   whole voyage. This is the loop where research decides whether the ship works.
3. **Material recovery per product life.** At 95% recovery per 20-year life, 8% of a material
   survives a thousand years; at 99.9%, 95%.
4. **Fusion**, the operator's first named option, matters for the **fuel stock and the waste**: a
   kilogram of deuterium-tritium gives about 4.1 times what a kilogram of fission fuel does, and
   leaves no long-lived actinides. It does not change the ship's power, heat or radiators, because
   those are set by how much energy each person's food takes. Fusion has ignited at the target
   (8.6 MJ from 2.08 MJ of laser light, April 2025) but has not yet sent net electricity to any grid
   (readiness 3 to 4).

**Two things in this repository did not balance**, and one was fixed (section 12):
`data/machines/home_solo.ron`'s Nutrients loop said it closed; it does not, and the family home
and the outline had already said so. And **no single "percent closed" number by mass means
anything**: a home that recycles its water and air and grows no food at all is 99.1% closed by
mass, because water dominates the mass (section 1). The test refuses to print one.

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
  about 400 mSv a life; unshielded deep space 1.8 mSv a day, past NASA's 600 mSv career limit in one
  year).
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

- **Genetics.** Marin and Beluffi found 98 people enough to survive a 6,300-year voyage in their
  demographic model, with inbreeding forbidden (marin-2018). Smith's genetic review asks for 14,000
  to 44,000 founders, 40,000 (an effective population of 23,400) as the safe figure, for health over
  five generations (smith-2014). They disagree by two orders of magnitude because they ask different
  questions (survive at all, against stay healthy); the test carries both.
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
- Shielding per person falls as a habitat grows; **heat rejection does not**. At 290 K the lore's
  hull side (157,080 km2, 60,600 sq mi) holds radiator for about 12 billion people as fins of its
  area, about 6 billion laid flat on it. The ship is near its heat limit, not its space limit.

**Earth, if everyone lived on a self-sufficient homestead.** The outline's homestead needs 0.28 to
0.34 ha (0.7 to 0.8 acre) a person at the bare tier: 2,300 to 2,800 million ha for 8.2 billion
people, about cropland (1,600) plus half again, under all agricultural land (4,800). At the comfort
tier, heated by a woodlot, 0.83 to 1.46 ha: 6,800 to 12,000 million ha, more than all agricultural
land and close to all 13,000 million ha of land on Earth. Self-sufficiency for everyone is a land
question before it is a technology question, and the house is the lever (the woodlot is most of it).

---

## 5. Scenario A: Earth, today, real, now

Every number below is in `earth_today.ron` with its quotation. Population 8.2 billion in 2024,
peaking near 10.3 billion in the mid-2080s (wpp-2024).

**Energy (2025, Energy Institute Statistical Review 2026).** Total energy supply 600.31 EJ (569
quadrillion Btu), an average of 19.0 TW: 72.9 GJ (69.1 million Btu), 2.3 kW, per person. Oil 200.97
EJ, gas 150.70, coal 166.03: 86.2% fossil. Nuclear 5.2%; hydroelectricity and renewables 51.57 EJ,
8.6%. Electricity 32,202 TWh, 3,930 kWh a person a year, 450 W each on average. The review counts
wind and solar at the electricity they make; an accounting that credits them with the fuel they
displace shows a larger share.

**Air (Global Carbon Budget 2025).** Human emissions 11.6 GtC (42.4 Gt of CO2) in 2024, about 5.1 t
(5.7 short tons) of CO2 per person; the land and ocean took up 21% and 29% over the past decade.
Atmospheric CO2 425.7 ppm in 2025. The oxygen loop is closed by the biosphere: what people breathe
out (about 2.2 Gt of CO2 a year) is this year's food carbon.

**Water (SDG Extended Report 2026, AQUASTAT).** Withdrawals 4,022 km3 (965 cubic miles) in 2023,
nearly flat since 2015: agriculture 71.7%, industry 14.9%, municipal 13.3%. That is 1,343 L (355
gal) a person a day, 179 L (47 gal) of it municipal, against 43,000 km3 of renewable freshwater.
Water stress averages 18% and passes 75% in Northern Africa and Western, Southern and Central Asia.
Of 332 km3 of household wastewater, 185 km3 (56%) was safely treated in 2024.

**Food (FAO, UNEP).** Dietary energy supply 3,016 kcal a person a day in 2023, against a population
need of about 2,100 (the planning figure for whole populations, enn-1997). 13.3% is lost between
harvest and retail (2023); 1.05 billion t, 132 kg (291 lb) a person, almost a fifth of what reaches
consumers, is wasted (2022).

**Land and soil (FAO).** Agricultural land 4,800 million ha (cropland 1,600, pasture 3,200), forest
4,050, other 4,150, in 2023. Cropland 0.19 ha (0.47 acre) a person, down from 0.24 in 2001. Topsoil
forms at up to 1,000 years per centimetre (0.4 in).

**Nutrients (USGS, 2026).** Phosphate rock mined 250 million t in 2025; reserves 73 billion t,
resources more than 300 billion; "There are no substitutes for phosphorus in agriculture." 47.8
million t of P2O5 (20.9 million t of phosphorus) went into fertilizer. 160 million t of nitrogen was
fixed as ammonia (from air, which is unlimited, with energy that is mostly fossil). Potash 49
million t, more than 120 years of reserves. People excrete about 5.0 kg (11 lb) of nitrogen and 0.75
kg (1.7 lb) of phosphorus a year each on a Finnish diet (viskari-2018): all of humanity's excreta
hold roughly a sixth to a third of fertilizer phosphorus, and about 10% of it is returned
(a secondary account of Cordell et al. 2009, unverified).

**Materials.** 106 billion t (117 billion short tons) a year, 39 kg (86 lb) a person a day (2020,
gro-2024); 6.9% recycled (cgr-site); 62 million t of e-waste, 22.3% formally recycled (2022); 18 of
60 metals recycled above half at end of life (2011, with data from 2000 to 2005).

### What "as close to 100% as possible with today's technology" would take

Every technology below operates somewhere today; what limits it is building it, not inventing it.

| Loop | Today | What closes it with operating technology | What still cannot close |
|---|---|---|---|
| Air | 50% of carbon returned | Electricity from wind, sun, rivers and nuclear for the 86% that is fossil; electrified heat and transport | Process emissions (cement) and land-use change need their own fixes |
| Water | 56% treated | Treat and reuse all wastewater; keep each basin's withdrawal under its renewable flow | Closure is local, basin by basin |
| Food | about 70% reaches people | Halve loss and waste; return food waste and excreta to farmland | Soil lost to erosion does not come back on a human timescale |
| Nutrients | about 1.5 to 3% of fertilizer P from people | Recover phosphorus from wastewater, sludge ash and urine (not sourced in this pass); ammonia from clean hydrogen | Phosphorus reaching the sea; about 290 years of reserves |
| Energy | 8.6% from flows | As Air | Equipment is still mined and manufactured |
| Materials | 6.9% recycled | Design for repair and recovery; collect e-waste | Material locked in buildings and roads; elements recycled below 1% |
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
m between decks. **Volume is not what limits this ship. Power, heat and minerals are.**

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
- **Light**: about 8 kW (3 to 15). Three routes (BIOS-3's 8.5 kW of light for full closure at
  today's LED limits, 11.4 kW; BVAD's measured biomass per kWh, doubled for modern LEDs, 1.9 to 20
  kW; first principles from photons per gram, 3.1 to 10.3 kW) and a fourth used to try to refute it
  (a modelled plant factory's 247 kWh per kg of dry lettuce, graamans-2018: 5.7 to 7.0 kW). It
  survived.
- **Air and water processing**: about 0.5 kW, mostly the fans that condense the crops' 200 L (53
  gal) a day of transpiration back out of the air, at the game's own measured 27 Wh per litre.
- **The household**: about 0.4 kW (the solo home's 8.7 kWh a day without life support).
- **Industry and repair**: about 0.5 kW, **an estimate with no source** (phase 2 replaces it).
- **Total about 9.4 kW, 225 kWh a day.** For comparison, Earth's electricity is 0.45 kW a person.
- **Backup machines** for the air if the crops fail: 7 to 9 kWh a day while they run.
- **Water**: at the ISS's 98%, a comfortable household loses 1.7 L (0.45 gal) a day, 0.6 t a year:
  5 billion t a year for 8.2 billion people. A thousand-year ship must return its brine and solids
  through its biological loop and lose only what leaks.

### 6.4 Heat
Every watt ends as heat. A fin radiating from both sides at 290 K (17 C, 62 F) with emissivity 0.9
sheds 722 W per m2 (67 W per sq ft): 13 m2 (140 sq ft) per person, 107,000 km2 (41,000 sq mi) for
8.2 billion, two-thirds of the hull's side. Laid flat on the hull it radiates from one side only and
needs 214,000 km2, more than the whole side. The reactors' own waste heat (about twice the
electricity) is rejected hotter, at 600 K about 13 kW per m2, 1.4 m2 more per person. Heat pumps
that raise the rejection temperature shrink radiators and cost electricity. The game does not model
any of this yet (ship-life-support.md section 5, Climate); it is the next rung of that work.

### 6.5 Fuel for a thousand years
- **Fission** (operating today; the game's reactor is a KLT-40S-class plant, data/ship_power.ron):
  74,000 to 90,000 t of heavy metal fissioned a year for 77 TW at 33 to 40% efficiency, 74 to 90
  million t over the voyage, which needs breeder reactors to use most of the uranium or thorium
  carried (not sourced in this pass).
- **Fusion, deuterium-tritium** (research): 17.59 MeV a reaction (from NIST masses), 337 TJ a kg,
  about 4.1 times fission's: 18,000 to 21,800 t a year, with 21,600 to 26,100 t of lithium-6 a year
  to breed the tritium, 18 to 22 million t of fuel over the voyage. The deuterium (about 8,700 t a
  year) comes from water, about 26 g a tonne.

### 6.6 Nutrients
The farm that feeds one person takes up about 2.6 kg (5.7 lb) of phosphorus a year
(data/home_outline.json). Kept at BIOS-3's 20%, 2.1 kg a person is lost each year: 17 million t for
8.2 billion people, close to the 20.9 million t of phosphorus in all of Earth's fertilizer. At 99%,
213,000 t a year; at 99.9%, 21,000 t a year and 21 million t for the voyage. There is no mine
between the stars, so this is the loop where the ship lives or dies.

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
| Crops under electric light, whole diet | Food | 5 to 6 | BIOS-3, Lunar Palace 365 (ground) | the 8 kW |
| Fission | Energy | 6 to 9 | a floating plant supplies Pevek since 2020; 1,000 years of breeding never done | fuel 74,000 to 90,000 t a year |
| Mass shielding | Habitat | 9 | the Earth's air | 4.5 t/m2 |
| Phosphorus recovery on Earth | Nutrients | not assessed | not sourced | none until sourced |
| **Fusion (D-T)** | Energy | **3 to 4** | NIF 8.6 MJ from 2.08 MJ on target (2025); ITER D-T planned 2039 | fuel mass about a quarter; power unchanged |
| Methane pyrolysis | Air | 4 to 5 | ground prototypes, theoretical more than 86% oxygen recovery | oxygen water draw 0.18 kg a day |
| Electrochemical oxygen recovery | Air | 3 | laboratory, aiming at 75% | none yet |
| **Microbial protein from hydrogen** | Food | 4 (8 on Earth) | a commercial plant; O2/CO2 2.7; 24 kWh/kg in a 2017 lab | food power about 5.8 kW if it is 30% of calories (a game choice) |
| Food grown on electrolytic acetate | Food | 3 to 4 | laboratory; about fourfold over photosynthesis in sunlight | none yet (no electricity per kg read) |
| LEDs at their limit | Food | 6 to 8 | 4.1 umol/J calculated limit | food power 6.6 kW |
| **Full mineral recycling (MELiSSA-style)** | Nutrients | 4 to 5 | pilot plant with animals | phosphorus 2.6 g a person a year |
| Active (magnetic) shielding | Habitat | 2 to 3 | rejected by SP-413 in 1977 | none |
| Medicine on demand; capturing matter on the way | Materials | not assessed | not sourced | none until sourced |

### 7.3 What matters most, and why fusion is not first

Rank the options by how much of the ship they change. **Food per kWh comes first** because lighting
is 85% of the power, and the power sets the radiators, the reactor size and the fuel. **Mineral
recovery comes second** because at today's measured figure the ship needs a planet's fertilizer
every year, and it is the only loop where the gap between today and what is needed is a factor of
several hundred. **Material recovery** decides which machines can still be rebuilt after a few
centuries. **Fusion** comes fourth for the ship as a whole: it is a better power plant (a quarter of
the fuel mass, no long-lived actinides, fuel from water and lithium), but it delivers the same
electricity as fission, so it leaves the power per person, the heat and the radiators exactly where
they are. For the operator's thousand-year frame it matters most for what is carried: 18 to 22
million t of fuel instead of 74 to 90 million t fissioned, and no centuries of spent fuel aboard.

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
| mothership_with_todays_technology | Aggregate | the aggregate tier | 9.4 kW, 77 TW, 13 m2, the phosphorus make-up |
| each_research_option_changes_only_what_it_says | Aggregate | the aggregate tier | each switch changes its figures and nothing else |
| simplified_mode_agrees_with_full | Calibration | the aggregate tier | the two modes never give different answers |
| a_thousand_years | Millennium | all of the above | the drift processes and the first stock to run out |

Each case lists its inputs, expected outputs with tolerances, its pass line, its compute and memory
budget, the systems it runs, and what must exist first. Several expected values are marked
**Report**: printed and recorded with no pass line, because nobody yet knows what right looks like
(the oxygen the solo home's crops make back, the food a year really grows), and pretending
otherwise would be a check that cannot fail.

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
| The ship's phosphorus | lost at (1 - retention) of 2.6 kg a person a year | 21 million t carried at 99.9%, 213 million t at 99% |
| Reactor fuel | at 77 TW | 74 to 90 million t fissioned, or 18 to 22 million t of D-T |
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

## 11. The refutation pass

Every load-bearing figure was put to a second source. The per-figure results are in each figure's
`check` (77 figures in `data/closed_loop/`: 28 survived a second source, 24 remain uncertain, 23 are
derived and were recomputed a second way, 2 are marked corrected, and further corrections were made
inside derived figures and budgets, listed below). The ones that carry the conclusions:

**Survived a second source.**
- NASA's breathing figures: BVAD 2022 and Hanford 2004, 18 years and two different crewmembers
  apart, agree within 0.9% per MJ of food burned; BVAD's mass balance closes to the gram.
- ISS water recovery 98%: NASA's release and the brine processor's own 2024 paper (which shares an
  author, so it corroborates rather than confirms).
- ISS oxygen recovery about half: four NASA and ESA documents.
- Crop area per person: BIOS-3's table against NASA's review.
- The 8 kW of crop lighting: a fourth, independent route (247 kWh per kg of dry lettuce in a modelled
  plant factory) lands at 5.7 to 7.0 kW.
- Earth's energy supply, fossil share and per-capita energy: the review's own totals against its per
  capita figure and press coverage; Earth's population against the review's implied 8.23 billion.
- Water withdrawals: the three sectors sum to the stated total within 0.05%.
- Cropland per person: FAO's 0.19 ha against the outline's homestead need of 0.17 to 0.21 ha.
- The deep-space dose: 1.8 mSv a day (SwRI) against 1.84 +/- 0.3 (a DLR record seen in a search).
- Microbial protein electricity, to an order of magnitude: the derived 31 to 34 kWh per kg against a
  2017 laboratory figure of 24.

**Corrected.**
- FAO food loss: this pass began from a remembered 13.2%; no FAO text read says it; 13.3% (2023) is
  the portal's.
- BIOS-3's closure table: the scan's layout garbled it; read from the raw text order, with two OCR
  errors ("SO" for 80, ".10" for 30) fixed by position.
- The excreta-phosphorus ratio: a unit slip printed 294; it is 0.29.
- Plus, inside derivations: the lighting route through BVAD's biomass per kWh (first written 2.5 to
  21 kW on the wrong biomass range; 1.9 to 20), the deuterium in water (a remembered ocean ratio
  replaced by NIST's stated value and range), and the radiators (a hull-flush radiator radiates from
  one side only, which turns "two-thirds of the hull" into "more than the whole hull").
- **Dropped**: a search result's claim that a methane pyrolysis prototype recovered "up to 75%" of
  the lost hydrogen was not in the paper's text, and is not used.

**Uncertain, and said so.**
- The 6.9% circularity rate: the report itself was not read; coverage disagrees on its trend.
- Lunar Palace 365's 98.2%: the measure is undefined on the page read; the preprint refused scripted
  reads.
- Phosphorus returned from human excreta (about 10%): only a secondary account of Cordell et al. 2009.
- Phosphate reserves: the USGS and Cordell readings, carried as a range of 50 to 292 years.
- Shielding: SP-413's 1975 calculation; no modern transport study read.
- Every population threshold above a household except the genetic ones and the chip supply chain.

**Still unknown** (phase 2 must source these before a test asserts them): recovery rates and
readiness of phosphorus recovery from wastewater; a leak law for a large hull; whether any material
carries a 50 km ring at 1 g; electricity per kg of food grown on acetate; how much of a diet
microbial protein can safely be; micronutrients in a grown diet; seed-bank regeneration; topsoil
erosion rates; per-year spares mass for life support; the industry share of a ship's power; the
population a clinic, a foundry or a fab needs; whether MELiSSA has supported a human crew.

---

## 12. What does not balance

1. **FIXED: the solo home's Nutrients loop said it closed** (`data/machines/home_solo.ron`,
   `closes: true`, "the same way the 3-person home's does"). The family home's loop has said it does
   not since the 2026-09-26 garden model, the outline's audit finds compost and excreta return about
   53% of the nitrogen, 75% of the phosphorus and 93% of the potassium, and cannot_close.ron's footer
   says nutrients only nearly close. Set to `closes: false` with a note saying why. The Home page's
   "N/M loops close" line for the solo home drops by one. No test asserted the old value.
2. **A single closure-by-mass number hides an open food loop** (section 1). Not a bug in the game,
   which reports loops separately already; a rule for the test's reports.
3. **The game's people breathe on Hanford 2004's figures, not BVAD 2022's.** Per MJ of food they
   agree within 1%, so nothing changes; the unit case holds them together so a future edit to either
   is caught.
4. **The mothership's dominant load is unmodelled**: the solo home's garden is sun-lit through glass
   in the game, but between the stars there is no sun, so every ship farm is lit by electricity, and
   that light is 85% of the ship's power and all of its radiators. The game models neither crop
   lighting at ship scale nor heat rejection (ship-life-support.md section 5, Climate).
5. **Already recorded and still true**: the solo home's Energy loop does not close in either mode
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
2. **The report** (`report.rs`): the per-loop table, dual units, never one number;
   `closure_is_never_one_number_by_mass`; `person_breathes_by_the_current_nasa_baseline`.
3. **The home-day cases, today, on the existing harness**: `solo_home_closes_what_it_closes`,
   `family_home_closes_what_it_closes`, `one_to_three_residents_per_person`,
   `time_to_harm_when_one_thing_fails`. These need no runner and give the first measured closure per
   loop of the real homes.
4. **The aggregate tier** (`aggregate.rs`): a per-zone ledger with `population: u64`, per-person
   coefficients measured by step 3; `aggregate_matches_home_at_n1`,
   `earth_today_reproduces_the_measured_world`, `population_ladder`,
   `mothership_with_todays_technology`, `each_research_option_changes_only_what_it_says`,
   `simplified_mode_agrees_with_full`.
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
| `data/closed_loop/sources.ron` | 53 sources, each with its own date, the date read, how it was read |
| `data/closed_loop/person.ron` | one person's day, two tiers, the radiation and space a body needs |
| `data/closed_loop/earth_today.ron` | Scenario A: the measured world and each loop's closure |
| `data/closed_loop/mothership.ron` | Scenario B: the ship, what has been done, what one person needs |
| `data/closed_loop/technologies.ron` | options today and in research, with readiness and what each changes |
| `data/closed_loop/population_steps.ron` | one person to humanity, and what does not scale |
| `data/closed_loop/drift.ron` | the 1,000-year processes |
| `data/closed_loop/test_cases.ron` | the phase-2 cases, budgets and needs |
| `data/machines/home_solo.ron` | the Nutrients loop corrected to `closes: false` |
