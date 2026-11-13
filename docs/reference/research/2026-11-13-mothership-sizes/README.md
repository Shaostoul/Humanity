# The mothership's sizes: a sourced research pass

**Dated 2026-11-13** (sources read 2026-11-12 and 2026-11-13). Research for
[the mothership master plan](../../../design/mothership-master-plan.md), sections 1.3 and 1.4: the
operator's questions of 2026-11-12 on the sizes of the places beyond the home, the hangar, housing
tiers, livestock, the closed loop, the drum's steel, shielding, power, clothing, the colony module
and the weather. Every number below names its source (a link, the source's own date and a short
exact quotation of the words that carry it) and a confidence. Numbers worked out here from sourced
inputs say "computed". A judgement says "judgement". **A negative finding** (no source, not
possible, not safe) is recorded as carefully as a positive one.

The detailed notes behind each section, with every finding, are the helper files listed at the end
([A](A-space-rows.md) to [J](J-water-and-rain.md)). This README carries the answers.

## Short answer

- **The planning sheet** ([data/ship/space_program.toml](../../../../data/ship/space_program.toml)):
  the hospital doubles to 0.6 m2 a person (modern hospitals are about 140 m2 a bed, against
  SP-413's 58) on two storeys, so the same floor; the hangar becomes four bays, one per DEFINED
  transport class; the private craft's parking spot grows from 150 to 200 m2 (the old spot was
  smaller than the craft); eight more rows now cite a source, and six say plainly that no source
  was found. Drum 1 still holds **7,781** at the default mix; with no homesteads the bow fills at
  **11,500** (was 14,900).
- **The hangar**: Class S shuttle (the intro's ride, an S-92-sized craft, 19 seats) needs a
  **1,140 m2** bay; Class M ferry (ATR 72-sized, 72 seats) **1,520 m2**; Class C freighter (the
  operator's C-130) **3,290 m2**; Class H heavy lift for one 100-tonne-plus vehicle **5,950 m2** of
  deck over two storeys. One sector needs only Bay 1 (1,140 m2); a full Drum 1 has **16,840 m2** of
  bays in the bow.
- **Housing tiers**, floor per person: barracks **6.7 m2** of sleeping floor (US Army), about 13 m2
  gross; dormitory **31 m2** gross a bed; co-housing a home of about **20 m2** a person plus about
  **14 m2** of common house per household; row house **19.8 m2** a person (UK space standard, 2-bed
  4-person house) on a plot of about **25 m2** a person; homestead food land **372 m2** a person
  in soil (Ecology Action) or **40 to 65 m2** under lamps (NASA).
- **A homestead plot of 34 x 55 m** grows a soil-fed household of 2.5 on biointensive yields and
  would let Drum 1 hold **10,875** instead of 7,781 (computed). The operator's 34 x 34 m garden has
  **no primary source** behind it (a negative finding).
- **Livestock with good lives** need real land: a pasture-raised hen **10 m2** of range (Certified
  Humane), a dairy goat about **4 m2** of shelter and yard plus pasture, a breeding rabbit doe
  **3 m2**, fish at most **20 to 25 kg/m3** (EU organic law), **4 bee colonies** per hectare of
  orchard. The sheet's 5 m2 a person for animals is a confinement figure.
- **The closed loop**: rock powder gives phosphorus, potassium, calcium, magnesium and trace
  metals, but **no nitrogen and no carbon**. Those come from carbonaceous (CI-like) asteroid rock
  (Bennu's has a carbon to nitrogen ratio of about 20) and from legumes fixing nitrogen (about
  **50 to 150 kg N per hectare per year**). BIOS-3 closed only about **40%** of its nitrogen.
- **The drum's steel**: at the coordinator's 200 MPa allowable (which already holds ASME's factor
  of 3.5 on tensile strength), air pressure alone needs **14.0 cm** of steel, not 13 (the steel's
  own weight under spin adds 19 MPa), and air plus 2.6 t/m2 of soil and buildings needs **17.5 cm**;
  with the shield on the drum, **23.6 to 27 cm**. Carbon composite does the same job at **2.5 cm**.
  Real mills roll plates up to **5.2 m x 28 m, 52 t**.
- **Shielding** (a correction): the project's 4.5 t/m2 gives about **55 mSv a year** behind water in
  deep space, not 5. **7 t/m2 of water (7 m) gives 16 mSv a year**, within the 20 mSv a year that
  NASA-computed settlement studies use. Drum 1's shell then holds **24.7 million tonnes of water**.
- **Power**: 10.8 kW a person holds (9.4 kW lights crops, at 2.8 umol/J, above the DLC floor of
  2.30). Drum 1 full draws **84 MW**; three RITM-200N-class reactors (55 MWe each, any two carry the
  load) fission about **110 kg of uranium-235 a year**. Put the reactors in unpressurised vaults
  where a leak goes to space. Photovoltaics inside the ship only recapture lamp light; starlight
  between the stars is about **6.5 microwatts per m2**.
- **Clothing**: basalt fibre is **not** for next-to-skin wear (its maker: "skin irritations are
  possible"); everyday cloth is linen, with wool and some hemp; flax gives about **1 t of long fibre
  a hectare**, so 8 kg of cloth a person a year needs about **80 m2** of flax field.
- **The colony module** (new, 2026-11-12): long-term genetic health needs an effective population
  of **1,000 or more** (Frankham 2014), which is about **2,000 to 10,000 people** depending on how
  breeding is managed; with strict managed breeding a crew of **98** survived a 6,300-year
  simulation. A gene bank adds insurance; no source puts a number on how far it lowers the floor.
- **Weather** (new): burning hydrogen is **not a water source** (the hydrogen and oxygen came from
  splitting water, at about 55 kWh per kg of hydrogen); the farms transpire about **30 m3 of water
  a hectare a day**, all of it condensed back out of the air. Rain falling from the spine lands
  **tens to hundreds of metres** to one side (computed), so watering is by booms, drip and mist.

## How this was read

- **Ten research helpers** each took a topic and wrote the notes in the files below, from primary
  sources (agencies, standards, makers' sheets, peer-reviewed papers, extension services). This
  README was written from their notes; its judgements are the coordinator's research agent's.
- **Robots, checked after the fact.** Some helpers did not read every host's robots.txt before
  reading. So on 2026-11-12 every host cited on a source line (164 hosts) was checked against its
  robots.txt for Claude-User, ClaudeBot and anthropic-ai. **Ten findings came from pages that
  refuse those readers and were removed** (marked REMOVED in the helper files; none of their
  numbers is used here): the US UFC 4-721-10 barracks PDF host (C1, C2), an Oregon State hemp
  trial (D32), a rabbit charity whose robots.txt could not be read (D16), an FAO abstract page
  (E23), a titanium sheet (F4), a carbon-fibre sheet (F9), a steel news site (F21), a
  Congressional Research Service page (G18) and a rail platform guide (A14). Three regulations
  read through eCFR's API (which refuses Claude-User) were re-read on the Legal Information
  Institute's copy of the same text and confirmed (G19 to G21).
- **Spot checks.** The quotations that carry the biggest decisions were re-read against their
  sources: the NSS/OLTARIS shielding table (section 7, read in full), Ecology Action's area for a
  complete diet (section 3), and 10 CFR 20.1301, 50.47, 100.11 and Appendix J (section 8).
- **Could not read** (the main ones; each helper file lists the rest): faa.gov (403, so the FAA
  heliport circular was read from a town's posted copy of the FAA PDF), OECD (403), the Sphere
  Handbook (its site refuses Claude), IATA's manual (paid), the IAEA's 2024 SMR booklet (paid),
  NASA BVAD 2022 for some tables, the Traverse et al. 2017 transparent-PV paper (login), the
  Moscow region ministry (did not resolve), the BattleTech Master Unit List (its robots.txt refuses
  Claude), Komatsu's and Caterpillar's truck pages (not found, timed out).
- **Units**: m2 is square metres. "a person" means one resident. t is a metric tonne.

## 1. The planning sheet's rows: real references

The sheet sizes the ship from one row per kind of space (master plan 3.12). The rows marked
"proposal" had no outside source. Here is each, with what the pass found and what changed.

### 1.1 The rows that were proposals

| Row | Before | What the sources say | After | Confidence |
|---|---|---|---|---|
| General Services (barracks, respawner, medbay desk) | 600 m2 per 2,500 | US Army troop housing: "a minimum of 72 square feet of floor space per person" (6.7 m2; MD0164, US Army Medical Department subcourse, undated, C3). UNHCR: "Minimum 4.5m2 to 5.5m2 indoor living space per person" in cold places (Emergency Handbook, undated, C5) | 600 m2 kept, now sourced: it sleeps about 90 people, 3.6% of those it serves | medium |
| Axial rail stations | 3 x 1,000 m2 | no station-size standard readable (the platform guide found refuses automated readers) | kept, still a proposal | negative |
| Spoke lifts (end caps) | 8 x 40 m2 | a 21-person, 1,600 kg lift needs a shaft about 3.0 x 1.9 to 2.0 m, 5.7 to 6.1 m2 (TK Elevator shaft table, undated, A15, values read by the fetch tool, no sentence) | kept; the shaft part sourced, the stair still a proposal | medium |
| Air plant | 0.3 m2 a person, at least 500 | no published floor or volume per person for life-support machinery found | kept, proposal | negative |
| Drum control room | 300 m2 | no published size for a plant control room or mission control floor found | kept, proposal | negative |
| Heavy industry | 6 m2 a person | general industry 36 m2 of floor a worker, range 18 to 60 (UK Employment Densities Guide, 2nd ed., 2010, A12: "Ratios are generally expressed as the number of square metres per employee.") | kept, now sourced: one resident in six at a heavy-industry job, twice an industrial country's share, because the ship makes everything (judgement) | medium |
| Freight tunnels and lifts | 1 m2 a person | warehouse densities found (70 m2 a worker, A12) but nothing for tunnels | kept, proposal | negative |
| Bridge | 800 m2 | no published bridge or flight-control-room floor area found | kept, proposal | negative |
| Arrivals hall | 1,500 m2 | IATA via the Port Authority of New York and New Jersey (Terminal Planning Guidelines, August 2013): "15 square feet per seated passenger, and 10 square feet per standing passenger"; about 3.5 m2 a passenger with baggage claim, waiting and a passport desk (B17) | kept, now sourced: about 5 m2 a passenger with circulation, so 250 people at once beside the desks | medium |
| Hangar bays | 2,000 m2 per 1,000 | sized from defined craft (section 2) | **four rows**: shuttle 1,140 m2 per 2,000; ferry 1,520 per 5,000; freighter 3,290 per 10,000; heavy lift 5,950 per 10,000; at least one of each | high (craft and clearances), the counts a proposal |
| Private craft parking | 150 m2 per 20 | a four-seat R44 is 11.7 m long, 10.1 m rotor disc (FAA AC 150/5390-2D, Appendix C, 1/5/2023); the FAA's clearance "is 10 feet (3 m) for ground taxi operations" (B3, B14) | **200 m2** (14.7 x 13.1 m with shared clearance) | high |
| Mech and vehicle bays | 600 m2 per 2,000 | one Class H vehicle (16 x 10 x 7.5 m, section 2) with a 4 m working margin is 432 m2; a fire station bay holds a 3.05 x 15.24 m truck in about 5.2 x 27.7 m (B16, the bay size from a search summary) | 600 m2 kept, now sourced (bay plus parts room) | medium-low |
| Drone bay | 300 m2 per 500 | the drone has no defined size yet (drone-mining.md) | kept, proposal | negative |

### 1.2 The community rows, checked against modern standards

These rows come from NASA SP-413 (1977). The pass checked each against today's references; only
the hospital changed.

| Row (SP-413, m2 a person) | Modern reference | Verdict | Confidence |
|---|---|---|---|
| Hospital 0.3 | About 4.3 beds per 1,000 people (OECD average 2021, Health at a Glance 2023, read only as a search summary, A1), close to SP-413's 5. But "Whole hospital area per bed is rising from 90m2 pre 1990 towards the 135–155m2 current in the USA and France" (BDP, Learning from French Hospital Design, May 2004, A3) | **0.6 on two storeys** (4.3 x 140 / 1,000; the storeys a proposal), the same drum floor | medium |
| Schools and the library 1.0 | UK Building Bulletin 103 (June 2014): primary 350 m2 + 4.1 m2 a pupil, secondary 1,050 m2 + 6.3 m2 a pupil, gross "between 142% and 145% of the net area" (A4). At 17 to 20% of residents of school age (judgement) about 1.0 to 1.2 | keep 1.0 | medium |
| (Library, inside schools) | "many have now returned to the 0.75 SF/capita level" (0.07 m2; Omaha Public Library master plan, 2017, A6) | fits inside the schools row | medium |
| Shops 2.3 | "the U.S. has 23 square feet of retail space per person" (2.1 m2) and "the U.K. and France each have five square feet per person" (0.46 m2; ICSC via PaymentsJournal, August 6, 2019, A7) | keep 2.3 (the US level, generous); 1.0 is the lever if the floor runs short | low-medium |
| Recreation 1.0 | Sport England's model gives about 4.2 badminton courts per 10,000 people (2018, search summary only, A8) | keep 1.0 | medium |
| Assembly 1.5 | no modern per-person figure found | keep | negative |
| Open space 10 | NRPA: a typical US agency has "10 acres of parkland per 1,000 residents" (40 m2; ParkMetrics, A9); Fields in Trust 24 m2 (2020, via a council's guidance); China's GB 50137-2011 at least 10 m2 of green space a person (A10, labels lost in extraction) | keep 10 as the floor; the WHO's 0.5 ha within 300 m rule already applies (3.13) | medium |
| (The "WHO 9 m2 per person") | **Not a traceable WHO statement**: papers repeat it citing each other; no WHO document found (A11) | a negative finding: never quote it as the WHO's | medium |
| Workshops 4 | light industry 47 m2 a worker, general industry 36 (A12) | keep 4 | medium |
| Offices 1 | 12 m2 of net floor a worker (A12) times 6 to 8% of residents | keep 1 | medium |

### 1.3 The tool before and after

`node scripts/space-program.js` before the changes (2026-11-12):

```
Space program for 500 people (1 sector of 500, 3 neighbourhoods; the ship's farms feed 460 on 1 level)

Homes
  Homestead    8%         40 people       16 homes      102,991 m2 of drum floor
  Cabin       22%        110 people      110 homes          805 m2 of drum floor
  Apartment   70%        350 people        4 blocks      10,000 m2 of drum floor

By zone                    needed         Drum 1 offers
  drum floor             0.206 km2       3.14 km2   fits (7%)
  spine decks            0.017 km2      0.769 km2   fits (2%, 340 decks)
  bow decks              0.009 km2      0.158 km2   fits (6%, 20 decks)
  end caps               0.000 km2   (lift shafts and stairs)
  radiator fin           0.034 km2   outside the hull (Drum 1's own hull is 3.53 km2)

Floor per person, all zones: 466 m2 (drum 413, spine 35, bow 19)
Drum 1 HOLDS 500 people at this mix.
Drum 1 holds at most 7,781 people at this mix.
Drum 1's shield (4.5 t/m2 over 3.53 km2 of hull): 16 million tonnes.
```

After:

```
Space program for 500 people (1 sector of 500, 3 neighbourhoods; the ship's farms feed 460 on 1 level)

Homes
  Homestead    8%         40 people       16 homes      102,991 m2 of drum floor
  Cabin       22%        110 people      110 homes          805 m2 of drum floor
  Apartment   70%        350 people        4 blocks      10,000 m2 of drum floor

By zone                    needed         Drum 1 offers
  drum floor             0.206 km2       3.14 km2   fits (7%)
  spine decks            0.017 km2      0.769 km2   fits (2%, 340 decks)
  bow decks              0.021 km2      0.158 km2   fits (13%, 20 decks)
  end caps               0.000 km2   (lift shafts and stairs)
  radiator fin           0.034 km2   outside the hull (Drum 1's own hull is 3.53 km2)

Floor per person, all zones: 488 m2 (drum 413, spine 35, bow 41)
Drum 1 HOLDS 500 people at this mix.
Drum 1 holds at most 7,781 people at this mix.
Drum 1's shield (4.5 t/m2 over 3.53 km2 of hull): 16 million tonnes.
```

At other populations (after): 100 people use 0.017 km2 of bow (10%), 1,000 use 0.026 (16%), a
full Drum 1 (7,781) 0.105 (66%), 10,000 use 0.130 (82%). The drum floor is unchanged everywhere;
the farms on two levels still let Drum 1 hold 9,031. The shield line still says 4.5 t/m2: section 7
says why it should be 7.

## 2. The hangar, sized from defined transports

The operator (2026-11-12, master plan 1.4): "single crew/pilot ships up to 100 crew or more",
"a C130 stratofortress transport spaceship" (the C-130 is the Hercules; the B-52 is the
Stratofortress), and transports "large enough for 100 tons or more per cargoed vehicle". So the
bow has one bay per CLASS, each class defined by a real craft's envelope (its length, span and
height), the bay sized by a real standard's clearance.

### 2.1 The classes

| Class | Defined by | Envelope (L x W x H) | Carries | Source |
|---|---|---|---|---|
| Private craft | Robinson R44 | 11.7 x 10.1 (rotor) x 3.3 m | 1 + 4 | FAA AC 150/5390-2D, Appendix C, 1/5/2023 (B3) |
| **S, shuttle** (the intro's ride) | Sikorsky S-92 | 20.9 x 17.2 (rotor) x 5.5 m | 2 crew, 19 passengers | same table: "S-92 ... 68.5 68.5 17.9 56.3" (feet; B1) |
| **M, ferry** | ATR 72-600 | 27.17 x 27.05 x 7.65 m | 72 to 78 seats | ATR marketing booklet, 2018: "Overall length 27.17 m", "Wingspan 27.05 m" (B10) |
| **C, freighter** | Lockheed Martin C-130J-30 | 34.37 x 40.41 x 11.84 m | about 20 t (payload medium confidence) | C-130J Fast Facts: "Length 112 ft 9 in/34.37 m", "Wingspan 132 ft 7 in/40.41 m" (H1) |
| **H, heavy lift** | a design: one 100-tonne-plus vehicle in a 20 x 11.2 x 8 m hold | 60 x 20 x 14 m (judgement) | one walker or one ultra-class haul truck | the cargo envelope below (H) |

**The heavy cargo.** The largest haul trucks: Liebherr T 284, "Gross vehicle weight (GVW) - 605.00
t", 242 t empty (Liebherr, undated, H6); BelAZ 7530 "Length - 13 390 mm Width - 7 820 mm Height -
6 650 mm" (BelAZ, undated, H5). Komatsu's and Caterpillar's pages could not be reached. The
envelope is taken as **16 x 10 x 7.5 m and 250 t** (judgement). No aircraft carries a whole haul
truck: the An-124's hold is 6.4 x 4.4 x 36.5 m for "Maximum payload, t (lb) 120 (264,600)"
(Volga-Dnepr booklet, undated, H3), too narrow and too low for a 10 m wide truck, which is why
Class H is a design of our own, not a real craft. **BattleTech's walkers**: only their 100-ton
class is known from the operator; their official size could not be read (the Master Unit List's
robots.txt refuses Claude), and their designs and names belong to their owners, so the game sizes
a class, not their machines.

**What carries 100 people**: no flown spacecraft. SpaceX's Starship Users Guide says "the Starship
crew configuration can transport up to 100 people" (2020, B12): a company's plan, not a
demonstrated capacity. Crew Dragon and Starliner fly 4 on NASA missions, the Shuttle flew 7, and
NASA's HL-20 study was designed for 10 (B5 to B9).

### 2.2 Clearances, and why helicopter rules still fit

- **Helicopter parking** (FAA AC 150/5390-2D, 1/5/2023): "The minimum distance is 10 feet (3 m) for
  ground taxi operations and the greater of 10 feet (3 m) or 0.28 D for hover taxi operations"
  (B14). D is the craft's overall length.
- **Landing area**: "FATO Length 1.50 D 1.66 D but not less than 100 ft (30.5 m)", safety area
  "0.42 D but not less than 30 ft (9.1 m)" (B13). Used as a keep-clear zone around the dock, not
  as floor.
- **Aircraft stands** (ICAO Annex 14, Volume I, 9th edition, July 2022, recommendation 3.13.7): "C
  4.5 m ; D 7.5 m ; E 7.5 m ; F 7.5 m" between a moving aircraft and anything else (B15).
- **Judgement**: these rules size for pilot error, rotor flex and downwash, none of which depends on
  gravity, so they still fit a non-spinning hangar. They do not cover thruster plume heat, drift or
  a craft that cannot stop dead: a docking craft's bay uses the larger of 0.28 D and 4.5 m.
- **Unloading**: a 6 m strip along the side for the ramp, baggage tug and walkway on S and M, a
  10 m apron at the rear ramp on C and H (judgement: the FAA circular gives no walkway width).

### 2.3 The bays

| Class | Bay (computed) | Area | Height | Per (proposal) |
|---|---|---|---|---|
| Private spot | (11.7 + 3) x (10.1 + 3) m, clearance shared | **200 m2** | one 15 m deck | one per 20 people |
| S shuttle | (20.9 + 2 x 5.85) x (17.2 + 2 x 5.85) = 32.6 x 28.9 m, + 6 m strip | **1,140 m2** | one deck | one per 2,000 |
| M ferry | 36.2 x 36.1 m (ICAO code C), + 6 m strip | **1,520 m2** | one deck | one per 5,000 |
| C freighter | 49.4 x 55.4 m (code D), + 10 m apron | **3,290 m2** | one deck (11.84 m craft) | one per 10,000 |
| H heavy lift | 75 x 35 m (7.5 m all round), + 10 m apron = 85 x 35 m | 2,975 m2 floor, 21.5 m high: **5,950 m2** of deck | two decks | one per 10,000 |

- **One sector (500)**: in practice only Hangar Bay 1, a Class S bay of **1,140 m2** (rung 1); the
  sheet counts one bay of every class for any ship, 11,900 m2.
- **Drum 1 full (7,781)**: 4 shuttle, 2 ferry, 1 freighter and 1 heavy bay, **16,840 m2**, plus 390
  private spots (78,000 m2), 4 mech bays and 16 drone bays: the bow is **66%** used.
- **Throughput is not the limit** (computed): a drum of 7,800 whose residents each fly out and back
  once a year makes about 43 passenger movements a day, under one ferry load. The bay counts are
  about having every class able to call, and spares.

### 2.4 Moorings for odd and oversized ships

- **Docking ports are small.** The International Docking System Standard's soft-capture ring
  "intersects the conic outline of the Guide Petals at a diameter of 1200 mm" (NASA, IDSS IDD Rev
  F, July 2022, H10); the Common Berthing Mechanism ring has an "inside diameter of approximately
  1.8 meters (71 inches)" (NASA NTRS 19920015843, 1992, H11). People and small cargo pass; no
  vehicle does.
- **A mast lets the ship swing free.** The airship principle: "Entire freedom of the airship in its
  horizontal and rolling movements" at a mast (NACA TM 513, 1929, H13).
- **Recommendation**: mooring masts on the bow's outer hull, each a boom with an IDSS-type port for
  people, a CBM-size port for cargo pods, and power, water and air lines. A moored ship floats free
  on its mast and turns with it. Anything bigger than a cargo pod (a vehicle) goes through a Class H
  bay. How many masts is the operator's (Decisions).

## 3. Housing tiers

The operator (2026-11-12): "barracks would be the most extreme version of shared housing ...
Then at the other end of the spectrum is like 1 person and the minimum necessary for them to
self-sustaining with plants/animals."

### 3.1 The ladder

| Tier | People per unit | Floor per person | Source | Drum floor a person (computed) |
|---|---|---|---|---|
| 0. The floor under all | | 3.5 m2 (warm) or 4.5 to 5.5 m2 (cold) of covered living space | UNHCR: "Minimum 3.5m2 of covered living space per person in tropical or warm climates, excluding cooking facilities or kitchen." (C5) | never below this |
| 1. Barracks | a bay of about 50 | 6.7 m2 sleeping floor ("a minimum of 72 square feet of floor space per person", never below 55 sq ft, 5.1 m2; US Army MD0164, C3); about 13 m2 gross with washrooms, day room and circulation (computed: 6.7 x 1.4 x 1.1 plus 3 m2 shared) | C3 | about 6.5 on two storeys |
| 2. Dormitory | halls of 100 to 250 | 31 m2 gross a bed, lounges and kitchens included: "space allocated per bed in the median project was 333 square feet" (College Planning and Management, 2014 College Housing Report, C7) | C7 | about 10 on three storeys |
| 3. Co-housing cluster | 12 to 36 households | a private home (own kitchen and bath) of about 20 m2 a person plus about 14 m2 of common house per household. "Anywhere from 12 to up to 36 households seems to work best" (CNU cohousing guidelines quoting McCamant and Durrett, undated, C9); the common-house figure is from practitioners' posts read only as summaries (C10, low) | C9, C10, C11 | about 15 to 20 on two storeys, plus its shared garden |
| 4. Row house | 4 people a house | 19.8 m2 (UK Nationally Described Space Standard, 2-bed 4-person 2-storey home, 79 m2; its bedrooms "at least 7.5m2", C11); a private garden of at least 25 m2 ("At densities above 50 dwellings per hectar an outside space of at least 25 sq m is expected for all homes", Essex Design Guide, C13) | C11, C13 | plot 5 x 20 m, 25 m2 a person; US zoning allows lots 14 ft wide (Raleigh R-10, C12) |
| 5. Homestead | a household | the house plus the land that feeds it (3.2) | C14, C18, C19 | 714 m2 for one, 1,870 m2 for 2.5 (recommended plots, 3.2) |

Today's sheet has three home kinds (homestead, cabin, apartment). Barracks, dormitory, co-housing
and row house become home kinds of their own when the operator picks the ladder (Decisions).

### 3.2 How much land feeds one person

| Way of growing | Area for one person's whole diet | Source | Confidence |
|---|---|---|---|
| Soil, biointensive beds | **372 m2** (4,000 sq ft): "A complete diet for one person realistically takes 4,000 sq ft, when grown at GROW BIOINTENSIVE® intermediate yields." (Ecology Action FAQ, undated, re-read 2026-11-12); "as little as 2,000ft2" (186 m2) with good soil and skill (Spring 2024 newsletter) | C14 | high that it is their figure; a vegan diet |
| Field farming, US diets | 1,300 to 10,800 m2: "Annual per capita land requirements ranged from 0.13 to 1.08 ha person-1 year-1 across the ten diet scenarios." (Peters et al., Elementa, 22 July 2016) | C15 | high |
| Under lamps, calories | 40 to 50 m2: "About 40-50 m2 of crops grown under high light intensities would be needed to produce enough dietary calories for one human" (NASA review, Frontiers in Astronomy and Space Sciences, November 2021) | C19 | high |
| Under lamps, NASA's baseline | 65.29 m2 of crops, all crops (BVAD Rev 2, February 2022, "Total 1.35 0.07 19.50 4.29 65.29 6.66") | C18 | high |
| Under lamps, every nutrient | 122.2 m2 with paths (the project's comfortable garden, survival-garden.md) | project | (project's own) |
| A sealed test | about 250 m2 of crops a person gave about 80% of the food in Biosphere 2 (Nelson and Dempster 1996, E16) | E16 | high |
| **The operator's 34 x 34 m (1,156 m2)** | **no primary source found** for 1,156 m2 or for "34 x 34 for one person"; it is about three times Ecology Action's figure, and a quarter acre (1,012 m2) is not the same number | C17 | **a negative finding** |

**Recommended homestead plots** (judgement, computed): one person in soil needs about 372 m2 of
beds, a 20 m2 house, a few hens (6 x 10 m2 of range, section 4) and paths: a **21 x 34 m plot (714
m2)**, a Fibonacci rectangle. A household of 2.5: 930 m2 of beds and the rest, a **34 x 55 m plot
(1,870 m2)**. Today's 55 x 89 m plot (4,895 m2) is 2.6 times that. With 34 x 55 plots and a 10 m
street and gap, **Drum 1 holds 10,875 at the default mix instead of 7,781**, a sector made only of
homesteads needs 0.61 km2 instead of 1.32, and Drum 1 holds 2,600 homesteaders instead of 1,187
(computed with the sheet). The shipped homestead's lit greenhouse (990 m2) already fits a 34 x 55
plot.

**Tatami** (the operator likes tatami mats as a module): common sizes are given as Kyoma 955 x
1910 mm, Chukyoma 910 x 1820, Edoma 880 x 1760, Danchima 850 x 1700 (real-estate glossaries, no
Japanese standard or government source found, C22); the 1.62 m2 per mat in property listings is an
advertising rule of the Real Estate Fair Trade Council, not a building-code module (C23, medium).
So **a tatami module does not make a design "to code"**; building-code minimums per country need
their own dated finding before any claim (CLAUDE.md).

### 3.3 Showcase sectors, one per style (the operator's tram-line idea)

Computed with the sheet, one 500-person sector each, farms and shared places included:

| Sector made of | Drum floor | Share of Drum 1 |
|---|---|---|
| Cabins (single rooms in halls) | 0.101 km2 | 3% |
| Apartment blocks around an atrium | 0.110 km2 | 4% |
| Barracks, dormitory, co-housing or row houses | about 0.10 to 0.12 km2 (computed from the cabin case: everything but the homes is the same 97,000 m2) | 3 to 4% |
| Homesteads, today's 55 x 89 m plot | **1.32 km2** | **42%** |
| Homesteads, a 34 x 55 m plot | 0.61 km2 | 19% |

So every style except the homestead fits a showcase sector easily; a full homestead sector would
take a fifth to two-fifths of the drum. **Recommendation**: the homestead showcase is one
neighbourhood (about 50 homesteads) beside the others, not a whole sector.

## 4. Livestock with good lives

The operator (2026-11-12): "We can't force them to live in tiny cages." The standards below are
certifiers' and the law's own text.

| Animal | Shelter | Outdoors | What it gives | Feed it needs |
|---|---|---|---|---|
| Laying hen | 0.14 m2 ("a minimum of 1.5 sq. ft. (0.14 sq. m) per hen", Certified Humane, D2); AGW 0.16, EU 0.17 | **10 m2**: "The minimum outdoor space requirement is 2.5 acres (1 hectare)/1000 birds." (Certified Humane Pasture Raised, Edition 21, 2023, D1); rotated | eggs: "The annual average production per layer on hand in 2024 was 290 eggs" (USDA NASS, February 2025, D6), about 250 on pasture (judgement); insects eaten under trees | about 45 kg a year (100 to 150 g a day, Alabama Extension, September 2022, D7) |
| Dairy goat | 1.5 m2 inside, 2.5 m2 yard (EU 2020/464, Annex I, 26 March 2020, D9); about 4 m2 lying and loafing (AGW, D8) | pasture always: "Continuous outdoor pasture access is required for all goats." (AGW, D8); no certifier gives an area: about 1,000 m2 (judgement, low) | milk: "Alpines produce 2,715 pounds of milk during an average lactation" (1,230 kg; Mississippi State Extension, D10) | about 800 to 900 kg dry matter a year (D11, low) |
| Sheep | as the goat (EU, D9) | pasture, about 1,000 m2 (judgement, low) | wool about 3.2 kg greasy a year (USDA NASS, 31 January 2025, D12) | as the goat |
| Rabbit (breeding doe) | 0.6 m2 inside + 2.5 m2 outside, 3 m2 a doe (EU 2020/464, Annex I Part V, D15) | | about 30 young a doe a year (D17) | about 3.75 kg feed a kg of live weight (D17) |
| Fish (tilapia, trout) | | at most "Oreochromis: 20 kg/m3" and "Brown trout and Rainbow trout 25 kg/m3" (EU organic aquaculture, Annex II, 26 March 2020, D18); run at 10 to 12 kg/m3 (judgement) | fish; the water feeds plants (aquaponics) | 1.5 to 1.9 kg feed a kg of tilapia (D19, search summaries) |
| Honey bees | a hive | **4 colonies a hectare** of fruit trees: apples "1.5" hives an acre (PNW 623, Oregon State Extension, January 2011, D20) | pollination; honey about 23 kg a colony a year (USDA, search summary, D21) | their own forage |

- **Manure nitrogen**, as excreted: a hen about 0.6 kg N a year, a sheep or goat about 10 (USDA NRCS
  handbook, March 2005, D14, medium): the nitrogen loop of section 5 runs through the animals.
- **Pollination matters**: pollinator-dependent crops make about 35% of world crop volume (IPBES
  2016, read only as a search summary, D22).
- **Against the sheet**: the sheet's animal area (5 m2 a person fed, SP-413) is a confinement
  figure. Pasture-raised animals live on the farm band's floor: hens and bees share the orchards
  (silvopasture: the same floor twice), fish share the water channels, goats and sheep need
  pasture of their own and are the expensive ones.
- **A modest herd for a sector of 500** (judgement): 250 hens in the orchards (2,500 m2 of range
  under trees), 10 dairy goats (about 1 ha of pasture and browse), 20 rabbit does, fish tanks, and
  4 colonies a hectare of orchard. Every animal's feed is grown aboard: 250 hens eat about 11 t of
  feed a year. The operator picks the herd (Decisions).
- **VR and treadmills instead of space** (the operator's idea): **no peer-reviewed study** of VR for
  farm animals was found; the 2019 Moscow region dairy trial was reported only by the press (the
  ministry's own page could not be reached); **no omnidirectional treadmill for livestock exists**
  in the literature searched (I9 to I11, negative findings). Welfare science points to real space,
  movement, enrichment and company (I12, reviews read as abstracts). So VR is not a credible
  substitute for space.

## 5. A closed loop fed only rock powder

The operator (2026-11-12): "How do we do this right so we create a closed loop ecosystem that only
receives raw biomatterless space rock powders."

- **What rock powder gives**: phosphorus, potassium, calcium, magnesium, iron and trace elements. One
  measured basalt: about 10% CaO, 12 to 13% MgO, 12% Fe2O3, 0.9% P2O5, 0.7% K2O (Frontiers in
  Climate, 2022, its Table 1 read as a summary, E1). "Beneficial results prevail for mafic and
  ultramafic rocks like basalts" (Swoboda, Doering and Hamer, Science of the Total Environment,
  2022, E2); typical doses 1 to 20 t a hectare, with nutrient imbalance at 50 to 100.
- **What it cannot give**: **nitrogen and carbon**. Silicate rock carries essentially none (inferred:
  no source states the zero in a sentence, E3).
- **Where carbon, nitrogen and water come from**: carbonaceous, CI-like asteroids. Bennu's samples
  are "like the Ivuna-type (CI) carbonaceous chondrites ... with a bulk C/N ratio of ~20", with
  nitrogen as ammonia (Sandford et al., Nature Astronomy, 2025, E4). Their water is bound in clay
  and needs heat to release (E5). So the ship's "rock powder" must include carbonaceous rock, not
  only basalt.
- **Nitrogen fixation**: legumes fix "<10 to >250 kg N/ha.year" (Crop and Pasture Science, read
  second-hand, E6); plan on 50 to 150. Free-living fixers manage only about 5 to 10 (E7).
- **How much nitrogen people cycle**: about 3.4 to 5.3 kg N a person a year (protein at 16%
  nitrogen, WHO/FAO/UNU 2007, read through summaries, E8): 1.7 to 2.7 t a year for a sector.
- **Closure is hard**: BIOS-3's third run closed "approximately 40%" of its nitrogen, and "None of
  the mineral elements in the ash was returned to the nutrient solution" (Salisbury, Gitelson and
  Lisovsky, BioScience, 1997, E9). MELiSSA's nitrifying step holds "ammonium conversion ... at
  99%" (Frontiers in Astronomy and Space Sciences, 2021, E13). Every waste stream (urine, faeces,
  ash, inedible plant parts) must return.
- **The helpers, seeded once at the start** (rock powder is lifeless, so the loop starts with a
  living seed of soil organisms):
  - **Pollinators**: bumblebee colonies "have subsequently replaced manual pollination" in
    European tomato greenhouses (Journal of Pollination Ecology, 2012, E10), 5 to 15 colonies a
    hectare of tomatoes; honey bees, 4 colonies a hectare of orchard (section 4).
  - **Decomposers**: composting worms (Eisenia fetida) in their "optimum range (59 to 77 degrees
    F)" (15 to 25 C; Alabama Extension, 2023, E12), springtails, fungi and bacteria.
  - **Predators for pests**: parasitic wasps (Encarsia) at "1/plant preventive" and "2-6/plant
    curative", predatory mites (Phytoseiulus) on infested leaves (UConn IPM, E11). Pests arrive
    on seeds and people even in a sealed system: Biosphere 2's ants and cockroaches (below).
- **Biosphere 2** (1991 to 1993): oxygen fell from 20.9% to about 14% in 16 months because "Oxygen
  was depleted from the atmosphere by reactions with organic C in the systems' soils" (Nelson and
  Dempster 1996, citing Severinghaus et al. 1994, E15), and the concrete took up the carbon
  dioxide; "Explosion of some populations including a species of ant and cockroaches" (E16);
  flying insects declined. Lesson: no rich organic soil sealed in without accounting for its
  carbon; seal or coat concrete; plan the insects.
- **BIOS-3** (Krasnoyarsk): crews of two or three for up to six months, about 63 m2 of crops (the
  sources differ), "80%-85% closure by recycling water" in 1968, about half the food (E18).
- **MELiSSA** (ESA): five compartments; "The target of the work is the creation of a closed loop
  life support system with close to 100% efficiency" (ESA, 2009, E19): a goal, not a result.

**Trees and wind.**

- **What wind does**: "the response of trees and other plants to bending is a reduction in
  extension/height growth" with "an increase in radial growth (secondary growth), which increases
  stem taper", and growth moved from shoots to roots (Telewski, Frontiers in Forests and Global
  Change, 2021, E20). The effect grows with the dose of flexing (Telewski and Pruyn 1998, Tree
  Physiology, abstract only, E21).
- **The Biosphere 2 story**: the popular claim that its trees lacked "stress wood" and fell over is
  **not in the crew's own 1996 paper**, which says only that canopies grew fast; it appears in
  press and blogs (E17). A negative finding: do not quote it as Biosphere 2's result.
- **How much air**: greenhouse practice keeps air "moving at 50 to 100 feet per minute" (0.25 to 0.5
  m/s; UMass Extension, E24). No source gives the air movement a tree needs to grow strong wood.
- **Recommendation** (judgement, low confidence): the orchards and parks get a steady 0.3 to 0.5
  m/s from the ventilation plus scheduled gusts of 3 to 6 m/s for 10 to 30 minutes several times a
  day, from the same fans; young trees are not staked rigidly. A tree-flexing test belongs in the
  farms' planning (Decisions).

## 6. The drum's structure

The operator (2026-11-12): "How would we construct the drum? What materials would we use? Let's
assume we're using steel. How thick should it be ... What would the dimensions of the metal sheets
be? Would we actually try bending the sheets or would we create them flat then weld them."

### 6.1 The formula

A spinning drum's rim is a ring in tension. Each square metre of wall carries its own weight at
1 g plus everything pressing outward on it (air, and the floor's load). For a wall of thickness t,
density rho, at radius R with g at the rim:

```
hoop stress  sigma = rho g R + P R / t          (P = air pressure + floor load per m2 x g)
so           t     = P R / (sigma_allow - rho g R)
```

(The project's drum-limits finding, F1, sets out the same formula from Jensen 2024.) For steel at
R = 250 m the wall's own weight is 7,850 x 9.81 x 250 = **19.25 MPa** before it carries anything.

### 6.2 Checking the coordinator's numbers

| Load on Drum 1 | Steel at 200 MPa (computed) |
|---|---|
| Air alone (101.3 kPa) | **14.0 cm** (12.7 cm if the steel's own weight is left out: that is the "about 13 cm") |
| Air + 1 t/m2 of contents (the sheet's assumption) | 15.4 cm |
| Air + 2.6 t/m2 (1 m of soil at 1.6 t/m3 plus 1 t/m2 of buildings and water; judgement) | **17.5 cm** |
| Air + 2.6 t/m2 + the 4.5 t/m2 shield ON the drum | 23.6 cm (the "about 20 cm") |
| Air + 2.6 t/m2 + 7 t/m2 of water ON the drum (section 7) | 27.0 cm |

**200 MPa is not "before a safety factor".** ASME's pressure-vessel rule takes "the lesser of
tensile strength divided by 3.5, or 2/3 of yield strength" (ASME VIII-1, read as a summary, F17).
For S690QL plate 100 to 200 mm thick (yield 630 MPa there, tensile from 710; ArcelorMittal
Industeel's declaration under EN 10025-6, November 2025, F2), that is 710 / 3.5 = **203 MPa**: the
200 MPa already holds a factor of 3.5 on breaking. NASA's rule for habitable modules is a factor of
1.65 on yield and 2.0 on ultimate ("Internal pressure only 1.65 2.0 1.5", NASA-STD-5001B with
Change 3, 2022-10-24, F18), which would allow about 385 MPa and halve the steel (8.7 cm for the
17.5 cm case). So the coordinator's figures are about right; the honest statement is **14 cm for
air alone and 17.5 cm with soil and buildings, at a conservative pressure-vessel allowable**.

### 6.3 Materials compared

Wall for Drum 1 with air and 2.6 t/m2 of floor load (computed with each material's working
strength by NASA's factors: the lesser of yield / 1.65 and tensile / 2.0; fibres at tensile / 2.0):

| Material | Strength | Stiffness | Density | Wall | Wall mass | What making it needs | Source |
|---|---|---|---|---|---|---|---|
| Steel S690QL | yield 690 (630 thick), tensile 710 to 940 MPa | about 210 GPa (not re-read) | 7.85 g/cm3 | 8.7 cm (17.5 at 200 MPa) | 681 kg/m2 | iron ore reduced with carbon or hydrogen: 24 GJ a tonne from ore, "Scrap-EAF ... 10.07" GJ a tonne from scrap (worldsteel, 2023, F10); by hydrogen "3.48 MWh of electricity per tonne of liquid steel" (Vogl et al., 2018, F11) | F2, F10, F11 |
| Steel S355 | yield 355 (295 thick), tensile 470 to 630 | | 7.85 | 27.6 cm at 134 MPa (ASME) | 2,169 kg/m2 | as above | F1 |
| Titanium Ti-6Al-4V | **not sourced this pass** (the sheet read gave its strength table as an image; another refuses automated readers) | | 4.47 g/cm3 ("Density: 0.160 lbs/cu. in.; 4.47 gm/cc", ATI, 2012) | | | the Kroll process: chlorine and magnesium; its energy only from search summaries (F13, low) | ATI sheet, read 2026-11-13 |
| Aluminium 5083-H321 (weldable, marine) | yield 228, tensile 317 MPa | 71 GPa | 2.66 | 24.1 cm | 642 kg/m2 | alumina by electrolysis: "around 70 GJ/tonne" (WEF with Accenture, 2023, F12); recycled about 5% of that | F5, F12 |
| Aluminium 2219-T851 (weldable, aerospace) | yield 317, tensile 427 | | 2.84 | 17.1 cm | 487 kg/m2 | as above | F6 |
| Basalt fibre composite | fibre "Tensile Strength 3000.0 MPa, Tensile Modulus 86 - 90 GPa ... Density 2.63 g/cm3" (Mafic roving listing, F8); composite strength not found: 60% of the fibre's (computed, as the project did for aramid) | 88 GPa fibre | about 2.06 (with resin) | 3.5 cm | 73 kg/m2 | basalt melted at about 1,450 to 1,500 C with nothing added; energy only from summaries (F15, low); the resin needs carbon | F8 |
| Carbon fibre composite (Toray T700S) | 2,550 MPa composite | 135 GPa | 1.56 | 2.5 cm | 39 kg/m2 | polyacrylonitrile from propylene and ammonia, so carbon AND nitrogen; about 300 MJ a kg (range 260 to 1,130; summaries only, F14) | project's drum-limits finding |

What it says (judgement):

- **Steel is the material a ship can make from asteroid iron** with the least: ore, a reducer
  (carbon or hydrogen) and electricity. It is the heaviest by far (681 to 1,377 kg/m2).
- **Aluminium** is lighter than steel for the same wall only in the strong grades, and its
  electrolysis costs about three times steel's energy a kilogram.
- **Carbon composite** is the lightest wall (39 kg/m2, a seventeenth of steel's) but needs carbon
  and nitrogen, the two elements section 5 says are scarce aboard.
- **Basalt fibre** is the in-between that fits a rock-fed ship: rock and heat make the fibre; only
  the resin needs carbon. Its composite strength needs its own source before it is designed with.
- **Recommendation**: steel for the first drum's rim and frames (simple, weldable, made from iron
  asteroids), with basalt-fibre composite studied for later drums; carbon composite where mass
  matters most (the spine, craft).

### 6.4 Plates: how big, and roll or facet

- **The largest plates real mills roll**: Dillinger, "Max. plate length 28 m Max. plate weight up
  to 52 t", 5.2 m wide (Dillinger Facts and Figures 2025, F19); ordinary structural plate "3,950
  mm max." wide (voestalpine, undated, F20). The largest plate-bending rolls take plates "over 300
  mm" (a 2021 press report, low) and about 150 mm on four-roll machines (F22).
- **A 17.5 cm plate 5.2 m wide** weighs about 7.1 t a metre of length, so a 28 m plate is about 200
  t: past the 52 t limit. Real drum plates would be about **5.2 x 7 m at 17.5 cm (50 t)**, about
  **300 around the drum** (computed: 2 pi x 250 / 5.2 = 302).
- **Roll or facet** (computed): a flat 5.2 m plate on a 250 m radius sags only 13.5 mm from the
  true circle (w^2 / 8R), so the drum could be a 302-sided polygon and look round. But bending a
  17.5 cm plate to R = 250 m strains it only 0.035%, far under steel's 0.33% yield strain, so the
  bend is elastic: a plate pulled into place stays stressed at about 73 MPa on its faces, and a
  rolled plate needs only a slight permanent set. A flat facet under 127 kPa between frames 5.2 m
  apart bends at about 56 MPa (fixed-edge strip, computed) and pushes its hoop load into a heavy
  frame at every seam. **Recommendation**: roll each plate to the drum's curve (a large radius is
  the easy case for bending rolls), so the shell carries pressure as pure tension; weld them into
  rings with ring frames behind. The operator's "sheets as the polygons" is right as the way the
  game MODELS the drum (302 facets), not as how it is built. How big cylinders are built today
  (API 650 tank shells from curved plates) was read only second-hand (F24, low).
- **Welding in space has been done**: "the first space welding and cutting of metals in their
  Soyuz 6 capsule using the automated and battery-powered Vulkan unit" (1969), and later
  aluminium and stainless welds in a space station matched ground welds (Sowards, NASA Marshall,
  Welding Journal, 2023, F25).

## 7. Shielding: water in sealed cells on a non-spinning shell

**The key source**: Globus and Strout, "Orbital Space Settlement Radiation Shielding", NSS Space
Settlement Journal, April 2017, computed with NASA's OLTARIS tool for a deep-space orbit at solar
minimum (17 June 1977 to 17 June 1978, "conservative"); read in full on 2026-11-12. Their Table 2,
effective dose in mSv a year behind water:

| Water, t/m2 (and depth) | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|---|
| mSv a year | 462 | 200 | 147 | 101 | 67 | 43 | 26.5 | 16.1 |

- Their finding: "6-7 tonnes of water or polyethylene radiation shielding per square meter of hull
  is sufficient in deep space" for their 20 mSv a year limit. On the 1970s figure the project
  uses: "Unfortunately, the 4.5 tonnes per square meter of hull estimate is quite low."
- **So the project's 4.5 t/m2 gives about 55 mSv a year behind water** (between the 4 and 5 t/m2
  rows; computed), and far more behind rock (their regolith column, 149 to 194). The 5 mSv a year
  that closed-loop-test.md attaches to it is not right. **This is a correction, not a choice.**
- **Measured, unshielded**: "The RAD data show an average GCR dose equivalent rate of 1.8
  milliSieverts per day in cruise" (Zeitlin et al. 2013, SwRI release, 30 May 2013, F31), about 660
  mSv a year.
- **Limits to aim at**: NASA's career limit, "total career effective radiation dose due to space
  flight radiation exposure shall be less than 600 mSv" (NASA-STD-3001 brief, Rev E, 27 December
  2022, F32); the public limit, "does not exceed 0.1 rem (1 mSv) in a year" (10 CFR 20.1301,
  confirmed on the LII copy) and ICRP 103's "1 mSv in a year" (F33). The NSS paper uses 20 mSv a
  year (the occupational limit) for settlers.
- **Recommended**: **7 t/m2 of water (7 m) as the home shield: 16 mSv a year**, sourced. Reaching 5
  mSv takes roughly 9 to 10 t/m2 and 1 mSv roughly 12 to 13 t/m2 (computed by extending the
  table's last rows: low confidence, since secondary particles flatten the curve at depth).
- **Storm shelter** for solar particle events: "The water wall should be at least 4 inches thick to
  provide effective SPE radiation protection in the deep space environment" (NASA Langley, RadWorks
  storm shelters, F30): a shelter room needs only 10 to 20 cm of water, because solar protons stop
  far more easily than cosmic rays.
- **Drum 1's water** (computed): 7 t/m2 over the 3.53 km2 hull is **24.7 million tonnes** (24.7
  million m3, a cube 291 m on a side); at 10 t/m2, 35.3 million. Held on a non-spinning outer
  shell, it loads the spine, not the drum (section 6).
- **Freezing**: "Ice at 0°C has a density of 0.917 g•cm–3, about 8% less than the density of water"
  (Introduction to Ocean Sciences, LibreTexts, F34), so water expands about 9% as it freezes. Each
  cell needs room to expand (a compressible liner or a gas pocket) or heat to stay liquid on the
  shadowed side. Ice shields about as well per tonne (the same hydrogen per kilogram; physics, not
  a sourced figure, F35). **Recommendation**: let the shell freeze and give every cell an
  expansion volume: a frozen shield needs no heating and cannot slosh or leak.

## 8. Power

### 8.1 How much

- **The project's 10.8 kW a person holds.** 9.4 kW of it lights the crops at 2.8 umol/J; the
  DesignLights Consortium's floor is "All products must have a PPE of at least 2.30" umol/J (V3.0,
  May 2024; its V4.0 raises it by 8.7%, G1). A plant factory needs 247 kWh of electricity a kg of
  dry lettuce against 70 to 211 in greenhouses (Graamans et al., Agricultural Systems, 2018, G4).
- **For scale**: the US used 12,711 kWh a person in 2023, 1.45 kW (World Bank series from IEA data,
  G6). The ship's 10.8 kW is 7.4 times that, because lamps replace the sun.
- **Industry**: steel from scrap about 0.66 GJ (0.18 MWh) of electricity a tonne in an arc furnace
  (worldsteel, F10); by hydrogen 3.48 MWh a tonne (F11).
- **Drum 1 full**: 7,781 x 10.8 kW = **84 MW**; a sector 5.4 MW (computed).

### 8.2 Which reactors

| Reactor | Electric power | Notes | Source |
|---|---|---|---|
| KLT-40S (Akademik Lomonosov) | 35 MWe each | "there can be up to 3.5 years between refuellings" | World Nuclear News (G8) |
| RITM-200N | 55 MWe from 190 MWt | "Fuel type: cermet fuel enriched to below 20%" | OKBM, OECD NEA workshop 2024 (G9) |
| NuScale | "77 MWe for each power module" | | US DOE, 30 May 2025 (G10) |
| BWRX-300 | 300 MWe | search summary only | G11 |
| eVinci (micro) | 5 MWe | "designed to run for eight or more full-power years before refueling" | Westinghouse (G12) |
| Kilopower / KRUSTY | "up to 10 kilowatts of electrical power" | NASA's space reactor test | NASA, May 2018 (G15) |

**Recommendation** (judgement): for Drum 1, **three RITM-200N-class reactors in three separate
vaults** (165 MWe; any two carry the 84 MW load, so one can be refuelled or fail). For a lone first
sector, two eVinci-class micro reactors (10 MWe for 5.4 MW). The sheet's KLT-40S pair rows already
give the same N+1 shape; changing the model is a decision (Decisions).

### 8.3 Fuel

- **Fission** (computed from physics: about 200 MeV a fission, 2.7 x 10^21 fissions a megawatt-day,
  IAEA reactor theory module, G16): about **1.05 g of uranium-235 fissioned a megawatt-day of
  heat**. Drum 1 at 84 MWe from about 290 MWt (RITM-200N's 29% efficiency) fissions about **110 kg
  a year**; the fuel loaded is far more, because only part of it burns before refuelling.
- **Fusion**: ITER says "a fusion plant with the same ouput will only require 250 kgs of fuel per
  year, half of it deuterium, half of it tritium" (ITER, "Fuelling the Fusion Reaction", undated,
  G17, its typo kept): about **21 kg a year for 84 MW** (computed). Tritium is "bred" when neutrons
  "interact with lithium contained in the blanket wall" (same page), so a fusion ship needs lithium
  and a tritium start-up stock. Helium-3 is too scarce to plan on (government figures read only as
  summaries; a negative finding for D-He3 as a first-generation plan). **Fission now, fusion
  later**, as the master plan says.

### 8.4 Siting, and the catastrophe case

The operator (2026-11-12): "Let's say something fails and a bunch of radioactive material leaks
from the reactor chamber into the main room absolutely flooding it with radiation. How do we
shield humans/animals/plants for that catastrophe?"

- **What the rules say on Earth** (re-read on law.cornell.edu, 2026-11-12):
  - the plume zone "shall consist of an area about 10 miles (16 km) in radius" (10 CFR 50.47(c)(2));
    for small reactors a zone may instead be drawn where dose is "projected to exceed 10 mSv (1
    rem) total effective dose equivalent over 96 hours" (10 CFR 50.33(g)(2)(i), G19);
  - at the exclusion boundary a person "would not receive a total radiation dose to the whole body
    in excess of 25 rem" in "two hours immediately following onset of the postulated fission product
    release" (10 CFR 100.11);
  - containment leak tests: "The leakage rate Lam shall be less than 0.75 La", La being the allowed
    "percent/24 hours" (10 CFR 50 Appendix J), set plant by plant at 0.2 to 2% a day (G21);
  - EPA's Protective Action Guide: shelter or evacuate at "1-5 rem (10-50 mSv) projected over four
    days" (EPA PAG Manual, 2017, G22).
- **A ship cannot have a 16 km zone** (the drum is 2 km long). So the protection is shielding and
  where the leak goes, not distance:
  - **Put the reactor vaults outside the pressurised volume, in vacuum**, in the spine's engineering
    segment, so a leak goes out to space and disperses instead of into anyone's air (judgement;
    the helper's G notes reach the same view).
  - **Shield by mass**: concrete cuts the gamma of caesium-137 tenfold every 15.7 cm and of cobalt-60
    every 20.8 cm ("Cesium-137 | 0.66 | 4.8 | ... | 15.7"; "Cobalt-60 | 1.17, 1.33 | 6.6 | ... |
    20.8", OSHA, G23). Two metres of concrete-equivalent cuts cobalt-60 gamma about 4 billion times
    (computed). Water does the same at about 27 and 36 cm a tenfold (computed from NIST, medium).
  - **Distance helps only by the square**: from 10 m to 100 m cuts dose 100 times, the same as 42
    cm of concrete (computed). So no "minimum distance" for homes and gardens does the job alone;
    the rule is **no home, garden or paddock inside the line where a vault's worst credible release
    projects 10 mSv over 96 hours**, met with mass. The drum floor is at least 220 m from the
    spine's surface anyway.
  - **Compartments**: each vault its own containment with a leak rate set tighter than the US
    plants' (0.1% a day, judgement); the bearing between spine and drum is an air break; the
    spine's air never reaches the drum (master plan 3.13).
  - **Shelters**: the drum's 7 m water shell and its soil are already shelter mass; every
    neighbourhood's node has an inner room behind the most mass (the storm shelter of section 7
    doubles as the radiation shelter), with its own air for four days, the PAG's window.
  - **Animals and plants**: paddock animals go to the shelters' barns; crops are expendable and
    replanted from the seed vault; the seed vault sits behind the most mass.

### 8.5 Photovoltaic surfaces

The operator (2026-11-12): "We can say all windows are solar panels. The same for all painted
surfaces."

- **Transparent PV**: "the limit for a visibly fully transparent single-band-gap solar cell is 20.6
  %, which is ca 2/3 of the 33.1 % theoretical limit" (Halme and Makinen, Energy and Environmental
  Science, 2019, G26); highly transparent cells are "recording efficiencies above 5 percent"
  (Michigan State University, G26). The original paper (Traverse et al., Nature Energy 2017) was
  behind a login.
- **Printed PV** ("painted"): the best research cells are perovskite 28.0%, organic 19.4% and
  quantum dot 19.1% (NREL's Best Research-Cell Efficiency table, updated 14 August 2026, G27): lab
  cells, not paint on a wall.
- **Inside the ship, panels gain nothing** (physics): every photon indoors came from a lamp the
  ship already paid for, and a panel turns at most a fifth to a quarter of it back into
  electricity while taking that light from the people or plants it was made for.
- **Between the stars, starlight is almost nothing**: "The total starlight energy density is uISRF =
  8.64 × 10-13 ergs cm-3" (arXiv astro-ph/9605046, 1996, citing Mathis, Mezger and Panagia, G28).
  On a flat panel that is about **6.5 microwatts a square metre** (computed: energy density x c /
  4), about 200 million times fainter than sunlight at Earth: Drum 1's whole hull would collect
  about 23 W of light.
- **Near a star it is real**: at Earth's distance from a Sun-like star, 1 km2 of 20% panels makes
  about 270 MW (computed). So windows and painted surfaces that are PV make sense on the OUTSIDE
  of the ship, used near stars, with fission between them.

## 9. Clothing fibres

- **Basalt fibre is not cloth for skin.** Its maker's safety sheet: fibres "have a nominal diameter
  of 10 - 22 micron", "At skin contact, skin irritations are possible", and "avoid wearing
  tight-fitting garments" (Basfiber SDS, revised 25 October 2017, D27). IARC: "Insulation glass
  wool, continuous glass filament, rock (stone) wool and slag wool are" not classifiable as to
  their carcinogenicity (Group 3; IARC Monograph 81, 2002, D29); basalt is not named, so the
  closest classes are a guide, not a ruling. Basalt is used for fire blankets, protective outer
  layers, insulation, composites and rebar.
- **Everyday cloth**: linen (flax) first, then wool and some hemp and cotton.
  - Flax: "about 1.0 tonnes long fibre/hectare" in 2025 on "5-5.5 tonnes of straw/hectare", but
    "400kg to 500kg per hectare" in the poor 2023 (European Confederation of Flax and Hemp, D31).
  - Cotton: "The 2022/23 world cotton yield is projected at 798 kilograms (kg) per hectare" (USDA ERS,
    April 2023, D33).
  - Wool: about 3.2 kg of greasy wool a sheep a year (USDA NASS, D12); a sheep's pasture makes wool
    the costliest per kilogram in floor.
  - Hemp's yield is not sourced this pass (the trial read refuses automated readers).
- **How much a person uses**: the world made "a record 124 million tonnes" of fibre in 2023 (Textile
  Exchange via nova-Institute, September 2024, D35), about 15.5 kg a person for every use
  (computed), most of it synthetic. **Design value 8 kg a person a year** of natural fibre
  (judgement), less with durable clothing and mending.
- **Farm area** (computed): 8 kg of linen at 1.0 t a hectare is **80 m2 of flax field a person a
  year** (200 m2 in a poor year), two-thirds of the comfortable garden's 122 m2. Halving the cloth
  (3 to 4 kg, clothes made to last and recycled fibre) halves it. Cloth is a farm row the sheet does
  not have yet (Decisions).

## 10. The colony module: how many people it takes (new, 2026-11-12)

The operator: "the colony module would be a single 'vehicle' or space city or something that
supports an entire population of humans infinitely without worrying about inbreeding comfortably."

- **The genetics rule**: "with Ne⩾100 being required to limit loss in total fitness to ⩽10%", and for
  keeping the ability to adapt over the long term "a better approximation is Ne⩾1000" (Frankham,
  Bradshaw and Brook, Biological Conservation, 2014, I2). Ne is the effective population, the
  number that actually passes on genes.
- **Ne is smaller than the head count**: in wildlife "Comprehensive estimates of Ne/N ... averaged
  only 0·10–0·11" (Frankham, Genetical Research, 1995, I3). Managed human breeding does better; no
  source puts a number on it (assumed 0.25 to 0.5, judgement).
- **For wild species** the median minimum viable population is "a median of 4169 individuals"
  (Traill, Bradshaw and Brook, 2007, I4).
- **For a crew under strict breeding rules**: "a minimum crew of 98 people is necessary" (Marin and
  Beluffi, arXiv 1806.03856, 2018, I1), for a 6,300-year voyage with planned pairings. The figures
  of 14,000 to 44,000 (Smith 2014) and about 160 (Moore 2002) could not be confirmed from their
  own pages (I5, I6).
- **Recommendation**: a colony module of **at least 2,000 people, planned at 5,000 or more** (Ne
  1,000 at a managed Ne/N of 0.2 to 0.5), with a frozen gene bank (eggs, sperm, embryos) as
  insurance. A gene bank lets a smaller start recover diversity later, but no source read gives the
  number it saves (low). A 500-person sector alone is too small to be genetically closed; Drum 1
  (about 7,800) is enough if people move between sectors. Human founder populations (Pitcairn,
  Tristan da Cunha) gave no sourced numbers in this pass.

## 11. Water and weather (new, 2026-11-12)

The operator: "if we burn hydrogen we generate water, right? What if we incorporate that some how?
Or should it just be emitted from sprinklers?"

- **Burning hydrogen makes water, but is not a source of it.** "If hydrogen is the fuel, the only
  products are electricity, water, and heat." (US DOE fuel cells page, J1); 1 kg of hydrogen makes
  8.94 kg of water (computed from NIST atomic weights, J3). But the hydrogen came from splitting
  water: today's electrolysers use about 55 kWh a kg of hydrogen, DOE's 2026 target 51 (DOE
  technical targets table, J2), about 6 kWh for each kilogram of water split. It is a loop that
  stores energy, not a new water supply. The ISS runs exactly this loop: oxygen from electrolysis of
  recovered water (J4), and "We have now demonstrated that we can reach total water recovery of
  98%, thanks to the brine processor." (NASA, 20 June 2023, J5).
- **How much water the farms move**: crops transpire about 2 to 7 mm a day in FAO's reference
  ranges (FAO Irrigation and Drainage Paper 56, Table 2, 1998, J6) and NASA's baseline gives 1.2 to
  4.7 kg a m2 a day (J8). **Design 3 mm a day, 30 m3 a hectare a day (peak 5 mm, 50 m3)**
  (computed). A full Drum 1's farms (about 87 ha) move about 2,600 m3 a day, every litre condensed
  back out of the air by the dehumidifiers, a cooling load of about 74 MW (computed with water's
  latent heat, about 2.45 MJ a kg, a textbook value not re-read). People add "1.42 kg/CM-d" of
  breath and sweat each (NASA BVAD, J9).
- **Rain in a spinning drum lands to one side** (computed: a drop in the drum's turning frame, with
  and without air drag, released at rest from a height above the floor):

| Released from | No air | A 2 mm raindrop (6.5 m/s fall, assumed) | Drizzle (2 m/s, assumed) |
|---|---|---|---|
| 3 m (a boom) | 0.3 m aside, 0.8 s | 0.3 m | 0.2 m |
| 10 m (a mast) | 2.0 m | 1.8 m | 0.8 m |
| 50 m | 27 m | 14 m | 4.7 m |
| 220 m (the spine's surface) | 135 m, 42 s | 233 m, 59 s | 76 m, 166 s |

  Drops land antispinward (behind the turn). SP-413 noticed the same for a jump: "he would not
  come straight down, but would land about 5.3 cm (more than 2 in.) to one side" (NASA SP-413,
  1977, J16). Raindrop and drizzle sizes: drizzle is "less than 0.5 mm (500 micrometers) in
  diameter", rain "more than 0.5 mm" (NWS glossary, J12); the fall speeds above are assumed (Gunn
  and Kinzer's 1949 table could not be read).
- **Recommendation**: water the farms with **booms, drip and misting near the plants** (aimed a
  little spinward); rain from the spine is a scheduled effect over parks (and a way to cool and
  wash), landing in a drifting band a minute after release, not the irrigation system. How much
  better drip is than sprinklers was not sourced (FAO says only that both are "generally higher"
  than surface irrigation, J18).

## Decisions for the operator

Each needs his choice; the recommendation is the research's.

1. **The housing ladder and its mix.** Which tiers exist (barracks, dormitory, co-housing, row house,
   homestead) and the share of residents in each. *Recommendation*: all five as home kinds, the
   default mix moving cabins into co-housing and row houses; the barracks as General Services only.
2. **The homestead plot.** Keep 55 x 89 m, or size it from the food it grows: 21 x 34 m for one and
   34 x 55 m for a household. *Recommendation*: 34 x 55 m (Drum 1 then holds 10,875, not 7,781),
   and a 21 x 34 m one-person homestead as its own home kind.
3. **The transport classes.** *Recommendation*: S (the intro's shuttle), M (ferry), C (the C-130
   class) and H (heavy lift for one 100-tonne-plus vehicle) as the bays; Starship-sized 100-person
   craft only as a later class.
4. **How many bays and masts.** *Recommendation*: one S bay per 2,000 people, one M per 5,000, one C
   and one H per 10,000, at least one of each; four mooring masts on the bow.
5. **The private craft and parking.** The R44-sized four-seater as the private craft (200 m2 a spot)
   and one for every 20 people (question 15 of the master plan). *Recommendation*: keep one per 20
   for Drum 1; the bow is 66% full at the default mix.
6. **The reactor model.** *Recommendation*: three RITM-200N-class units in three vaults for Drum 1
   (two eVinci-class micro reactors for the first lone sector), outside the pressurised volume;
   fusion later.
7. **The herd.** Which animals and how many. *Recommendation*: hens and bees in the orchards, fish in
   the channels, rabbits, a few dairy goats; sheep only for wool if cloth demands it.
8. **The shield.** *Correction, not a choice, but it moves the ship's mass*: 7 t/m2 of water
   instead of 4.5 t/m2 of rock (16 instead of about 55 to 170 mSv a year). His choice is the dose
   target: 20 mSv a year (7 t/m2), 5 (about 10 t/m2) or 1 (about 13 t/m2).
9. **The drum's material.** *Recommendation*: steel for Drum 1 (17.5 cm at a pressure-vessel
   allowable, about 4.3 million tonnes of rim), basalt composite studied for later drums.
10. **Cloth.** A flax field as a farm row (about 80 m2 a person) or imported fibre stores.
    *Recommendation*: a cloth row in the sheet, with 4 kg a person a year as the target (durable
    clothing), about 40 m2 of flax.
11. **The colony module's size.** *Recommendation*: 5,000 people (ten sectors) with a gene bank.
12. **Air movement for trees.** *Recommendation*: steady breeze plus scheduled gusts from the
    ventilation, and an orchard test to tune it.

## Corrections for the coordinator to carry

These are not choices; they are numbers the sources say are wrong elsewhere in the project:

- **The shield**: closed-loop-test.md, data/ship/space_program.toml (`[drum]` and `[scale]`
  `shield_t_per_m2`) and the master plan 3.16 use 4.5 t/m2 and say it gives 5 mSv a year. By the
  NASA-tool computation it gives about 55 mSv a year behind water. Moving to 7 t/m2 raises the
  ship's mass a person by about half (the shield dominates the mass, master plan 3.16) and Drum 1's
  shield from 16 to 24.7 million tonnes; the ship counts of 3.16 do not change (the spine's limit
  does not depend on the mass it carries). This pass did not change those numbers, because they run
  through the master plan's fleet tables.
- **The hoop numbers**: "about 13 cm of steel for air pressure alone" is 14.0 cm once the steel's own
  spin load is counted, and 200 MPa already holds a factor of 3.5 (section 6).
- **The WHO "9 m2 of green space a person"** must never be quoted as the WHO's (section 1.2).
- **Biosphere 2's "weak trees fell over for lack of wind"** must not be quoted as Biosphere 2's
  published result (section 5).

## Files in this folder

| File | What |
|---|---|
| [A-space-rows.md](A-space-rows.md) | community and support spaces per person |
| [B-transports-and-bays.md](B-transports-and-bays.md) | transport classes, clearances, parking, mech bay, Arrivals |
| [C-housing-tiers.md](C-housing-tiers.md) | housing tiers, land per person for food, tatami |
| [D-livestock-and-fibres.md](D-livestock-and-fibres.md) | livestock standards and yields; clothing fibres |
| [E-closed-loop-and-wind.md](E-closed-loop-and-wind.md) | rock powder, nitrogen and carbon, helpers, Biosphere 2, MELiSSA, BIOS-3, wind |
| [F-structure-and-shielding.md](F-structure-and-shielding.md) | materials, plates, design rules, water shielding |
| [G-power.md](G-power.md) | power, reactors, fuel, siting, PV, starlight |
| [H-heavy-transports-and-moorings.md](H-heavy-transports-and-moorings.md) | the C-130, the An-124, haul trucks, docking rings, masts |
| [I-viable-population-and-livestock-vr.md](I-viable-population-and-livestock-vr.md) | minimum viable population; VR for livestock |
| [J-water-and-rain.md](J-water-and-rain.md) | hydrogen and water, farm water, rain |

The sheet the numbers feed: [data/ship/space_program.toml](../../../../data/ship/space_program.toml)
(`node scripts/space-program.js`). Earlier findings this builds on:
[space program sources](../../findings/2026-11-06-space-program-sources.md),
[drum limits](../../findings/2026-11-07-drum-limits.md),
[community size](../../findings/2026-11-07-community-size.md).
