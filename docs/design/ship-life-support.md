# Ship life support: the home's air and the garden's water as closed loops

Written 2026-09-26 (built the same day). The operator's direction, 2026-09-27:
"We should focus on the space ship first but, we do need to get farming working
on the planet too. I imagine the spaceship gardens are a great way to figure out
all the physics, like the gasses, liquids, etc. to properly account for things."
The player's home is a station (`data/stations/home.ron`), so this rung makes
its air and water real closed loops around the garden.

Where things live:

| What | File |
|---|---|
| Every number, its source, a quotation and the date read | `data/life_support.ron` |
| The physics: gases, the exact balance, the machines' controllers, the home's own air | `src/systems/life_support.rs` |
| Each grow room's and tent's air, stepped with the home's | `src/systems/farming/humidity.rs` (`step_rooms`) |
| The crops' gas exchange and the irrigation's kept water | `src/systems/farming/mod.rs` (the crop loop) |
| The boundary test, the shipped homes' balance, the modes | `src/systems/farming/life_support_tests.rs` |
| The machines | `air_handler` and `air_recycler` (now the CO2 scrubber) in `data/machines/home.ron` and `home_solo.ron` |
| The mushrooms' air (2026-09-27): what stale air costs them, what each fungus breathes, the CO2 fans' controller | `data/garden/humidity.ron` (STALE AIR, THE FUNGI'S BREATH, THE CO2 FANS); `tent_co2_fan` and `room_co2_fan` in both home files |
| What the static power meter charges a day (2026-09-27) | `MachineDef::average_load_watts` in `src/machines.rs`; `average_watts` on the air machines in both home files |
| Who the electrical sim feeds first on a short island (2026-09-27) | `src/systems/electrical.rs` |

## 1. What was wrong before

- The home's air was held at 40% humidity forever. Everything the greenhouse's
  crops breathed out (about 541 L a day in the family home), the mushroom tents
  vented and the humidifiers made went into it and vanished.
- A 25 W "air recycler" made oxygen and scrubbed carbon dioxide, and occupancy
  drained 0.012 percentage points of oxygen a second for three people, so a
  power cut suffocated the household in minutes. Real people use about 2 kg of
  oxygen a day out of the several tonnes a sealed home holds.
- The irrigation billed the tanks per real day for water the air received per
  game day (BUG-092 item 7).
- The tanks kept their level in an f32. At 8,000 L its step is about 0.0005 L,
  and at 60 frames a second any net flow under about 0.9 L/min rounded to
  nothing every frame: a household tap's 0.17 L/min never left the cistern.

## 2. The model

**The airs.** Each grow room (a room a grow machine stands in) keeps its own
air; each mushroom rack's fruiting tent keeps its own inside its room; the rest
of the sealed home is the home's own air (`HomeAirState`, saved in
`SoilMemory`). Each holds water vapour and carbon dioxide as grams per cubic
metre, and the home's air holds oxygen too. Nitrogen is the rest.

**The balance** of one gas in one air, per hour: `dx/dt = s - SUM k_j (x - u_j)`.
The sources `s` are the crops, the people, the mushrooms and the humidifiers.
Each sink pulls toward a level `u_j` at a rate `k_j`: the air it exchanges with
(its leakage and fans), an air handler's coil (the vapour air leaving it holds),
a scrubber (zero), the leak overboard (zero). `life_support::relax` solves it
exactly over any slice, holding it under saturation (what goes over condenses on
the walls, counted) and above zero, and reports what each sink took, so the
air's water ledger closes to the gram. A catch-up is cut into slices of at most
a quarter of a game hour, because a crop's uptake follows its air's carbon
dioxide.

**People** breathe in proportion to the food they burn: Hanford 2004's crew
member (0.835 kg of oxygen in, 0.998 kg of carbon dioxide and 2.277 kg of water
out a day at 11.82 MJ) scaled to the household's Food loop (`food_demand_kcal`).
This is `data/home_outline.json`'s method, and at its 2,600 kcal it returns its
0.77 and 0.92 kg.

**Crops** take up carbon dioxide and give out oxygen through the pores they
breathe water out of, so the model ties them to that water: NASA's measured
grams per litre (BVAD 2022 Table 4-91; lettuce 5.1 g of CO2 a litre, wheat 6.5,
potato 11.3, tomato 13.1), in their lit hours at their light rate and their
health, the two things their growth follows. The rates were measured at 1,000
to 1,200 ppm; in thinner air a crop takes up less by Kimball's 33% per doubling
read as a logarithmic law (69% of the reference at 400 ppm, zero near 40 ppm).
Their tissue keeps water on top of what they breathe out (BVAD 2022 Table 4-90:
wheat 2.3%, lettuce 6.3%, tomato 10.0%), which the irrigation now draws.

**Mushrooms** breathe out their tent substrate's 1.09 g of carbon dioxide per kg
an hour (Pavlik 2020, already in `humidity.ron`) and take in oxygen for it at a
respiratory quotient of 1. Since 2026-09-27 only what is planted breathes, each
species at its own rate per "plant": an oyster or shiitake block 2.472 g an hour
(Pavlik's rate on a 5 lb block; the shiitake's is a game choice, no measured
rate was found), a square foot of button mushroom bed 0.929 g (IASRI's 10 g/h/m2
for the first two flushes). A tent used to breathe for its whole ten-block load
as soon as one shelf was planted.

**Stale air** (2026-09-27): a fruiting fungus in air past its CO2 limit grows
long stems and small caps and gives less (Cornell Small Farms; IASRI). The
model caps its health, the way it caps a fungus in dry air, at 100 x (1 -
severity x 0.267 x (ppm - limit) / 1000): Won 2010 measured the oyster's genus
at 102.4 g a bottle at 1,000 ppm and 75.1 at 2,000, a 26.7% loss. The limit is
1,000 ppm (Lin 2022, and Won's best), 1,500 for the button mushroom (IASRI).
The severity is the garden's Off / Gentle / Realistic setting (0, 0.5, 1), the
mode the pests, diseases and weeds follow. The sources disagree on where the
harm starts (Jang 2003 found a Korean bottle strain's best form at 0.3%); the
model takes the cautious end.

**The CO2 fans** (2026-09-27) are what growers do about stale air: an exhaust or
inline fan switched by a CO2 controller (the AC Infinity CO2 controller, sold by
mushroom suppliers for fruiting tents). Each rack's tent has an AC Infinity
CLOUDLINE S4 (226 CFM = 384 m3/h, 28 W) switched at 900 ppm, and each mushroom
room a CLOUDLINE S6 (425 CFM = 722 m3/h, 70 W) at 600 ppm. A CO2 fan is on for
the share of the time that lands its air on its setpoint (`co2_fan_duty`,
solved exactly through `relax`), and draws its watts for that share. Judged once
a slice like the humidity fans, the 384 m3/h fan changed a 1.73 m3 tent's air
two hundred times an hour and flushed it to its room's air, humidity and all.

**The air handler** is a Carrier 42CT size 14 ducted fan coil (1,842 m3/h at 50
Pa, 325 W), its coil on the station's cooling loop, the air leaving at the ISS
condensing heat exchanger's 5.9 C dew point (NASA ICES-2017-241). Pairing the
catalogue's fan with the ISS coil is a stated game choice: the catalogue's own
coil on 7 C water leaves its air at an 11.9 C dew point, too warm to dry a 20 C
home below about 60%. Its controller holds its air at 75% in a grow room (BVAD
2022's "about 75%" for plants) and 50% in the home's own air (the top of the
EPA's ideal 30 to 50%). Its draw follows the catalogue's three speeds, cycling
on Low below them (permanent split capacitor motors do not follow the fan laws).
Its condensate goes back to the cistern through its plumbing island.

**The CO2 scrubber** is the ISS Carbon Dioxide Removal Assembly: 4.74 kg a day
at a 2 torr inlet (Peters 2017), 860 W average (Hanford 2004 Table 7.3.2),
venting overboard. It runs only above 2,636 ppm (0.267 kPa, BVAD 2022's coming
crew limit) and takes a fixed share of the carbon dioxide in the air it draws.

**Leakage**: BVAD 2022's 0.02 kg of air a day per module, each of the home's
rooms counted as one (23 in the acre, 0.46 kg a day).

**Modes** (the house rule for deep systems): Settings > Gameplay > Ship life
support. Station-supplied, the default, has the station's own plant power the
air handlers and the scrubber; Realistic puts their draw on the home's grid.
The air, the water and the carbon are modelled identically either way.

**On a short island** (2026-09-27, found by the review of v0.1377): every solar
island is short at night (no sun from 18:00 to 06:00, the wind 150 W, and the
batteries do not yet carry load). The electrical sim then fed its loads
priority 5 first, so the CO2 scrubber (priority 1, "shed last") was the first
thing off; and it shed every load drawing 0 W, so an idle air handler, or one
the station powers, went off at night and stayed off, because the air step only
drove a powered unit. Now the loads are fed priority 1 first, a load drawing
nothing is never shed, and every air machine writes its draw every step: a
powered one what its controller runs it at, a shed one what it would draw with
power, so the island sees its real request (a shed handler used to keep asking
for its 325 W spawn nameplate in the Station-supplied mode, 3.1 kW of phantom
load on the family grid).

**The static power meter** (the Construction page's Usage and Buildability
figures, `MachineHome::utility_meters` and `buildability_report`) charged every
consumer its full draw for 24 hours: each air handler 7.8 kWh a day in either
mode, the stove 28.8. Since 2026-09-27 it charges each machine what it takes
from the home's grid averaged over a day (`MachineDef::average_load_watts`): a
ship life support machine nothing in the Station-supplied mode and its measured
`average_watts` in the Realistic one; a controller-driven machine its measured
`average_watts` (the shipped-homes test holds each figure to the measured draw);
a work station its idle draw, with its working draw named in the summary instead
of charged for the day; a grow light its timer's 6 hours.

**Cost**: one pass over the airs per tick (a few dozen rooms and tents, a few
machines each, a few `exp()` each); gameplay state only, nothing drawn or
networked.

## 3. The boundary between the two clocks

The air runs on game hours; the tanks on the plumbing sim's real minutes. Which
is right is the operator's open question (PRIORITIES blocked 3; Brief 6 in
`decision-briefs.md` lays out the options), and this rung does not choose.

Every flow crosses as **litres a day on both sides, taken from one physical
state**: the irrigation draws what the crops breathe out plus what their tissue
keeps, plus the humidifiers' mist; each air handler hands back the litres a day
its coil condenses, as a producer on its plumbing island. Each side applies those
daily figures on its own clock. Because they are the same day's figures, a day's
water balances on the tank side exactly as it does on the air side:

    litres drawn for the garden = litres the air handlers return
                                  + litres the crops keep + litres lost

The clocks only decide how long a day lasts. The air's own store (the vapour it
holds, a few hundred litres at most) fills and empties on the air's clock; at
steady state it does neither. Brief 6's proposed first increment (scale a crop's
water and air with its growth speed) multiplies both sides of the boundary alike
and keeps it balanced.

`the_gardens_water_balances_across_the_two_clocks` proves it: a greenhouse of
lettuce in a sealed home settles, then over one game day the cistern moves by
exactly the litres that crossed; per day drawn 108.79 L = returned 102.39 +
kept 6.40 + lost 0.0001; the air ledger closes to the gram; and at twice the
game speed the same day moves the same litres.

## 4. The balances, per home

Measured by `the_shipped_homes_hold_their_air_and_return_their_water` over a
game day and night at full planting after a day to settle (the home's own air
at 50%; the 60% column from the ignored
`print_the_shipped_homes_with_the_home_at_60_percent`). Before this rung both
homes' loops treated the home's air as free. Re-measured 2026-09-27 with the
mushrooms' CO2 fans (section 7); a figure that moved shows what it was.

### Family home (three residents, 6,600 kcal)

| Water, litres a day | |
|---|---|
| Into the air: greenhouse crops / court towers | 541 / 106 |
| Into the air: tents breathed / humidified / household | 7 / 29 (was 6) / 5 |
| Condensed and returned: greenhouse (2 units) / court (1) / home (4) | 357 / 75 / 257 (was 234) |
| Total returned, of 689 put in | 689 (was 666; 0 on walls, 0.01 leaked) |
| Drawn by the irrigation | 780 (was 757) |
| of which the outdoor fields' (into the weather) / the crops' tissue | 57 / 40 |
| Garden's net draw on the tanks | 91 |

| Air, kg a day (measured day) | |
|---|---|
| Carbon dioxide out: people / mushroom tents | 2.33 / 3.56 |
| Carbon dioxide taken up by the crops | 4.99 (rising with the air's CO2; it settles near 675 ppm, measured, when uptake meets 5.9) |
| Oxygen given out by the crops / breathed in (people 1.95, tents 2.59) | 3.60 / 4.54 |
| Home air CO2 over the day | 443 to 509 ppm (scrubber idle) |
| Mushroom room / tents CO2 | 600 to 604 / 900 ppm (was about 1,005 / 1,570 to 1,640) |
| Air leaked overboard | 0.46 |

| Energy | Home at 50% | Home at 60% |
|---|---|---|
| Air handlers, average | 1,574 W, 37.8 kWh a day (was 1,496, 35.9) | 965 W, 23.2 kWh a day (was 924, 22.2) |
| Tent humidifiers | 122 W (was 26) | 89 W (was 14) |
| CO2 fans: the room's / the six tents' | 51 W / 10 W | 51 W / 10 W |
| Exhaust fan, CO2 scrubber | idle, idle | idle, idle |

### Solo home (one resident, 2,200 kcal)

| Water, litres a day | |
|---|---|
| Into the air: greenhouse crops / court towers | 199 / 43 |
| Into the air: tents breathed / humidified / resident | 2 / 8 (was 4) / 2 |
| Condensed and returned: greenhouse (1) / court (1) / home (3) | 15 / 12 / 227 (was 224) |
| Total returned, of 254 put in | 254 (was 250) |
| Drawn by the irrigation | 297 (was 293; fields 29, tissue 15) |
| Garden's net draw on the tanks | 43 |

| Air, kg a day | |
|---|---|
| Carbon dioxide out: resident / tents | 0.78 / 1.19 |
| Taken up by the crops | 1.85 at 395 to 423 ppm (settles near 465 ppm, measured) |
| Oxygen given out / breathed in | 1.33 / 1.51 |
| Mushroom room / tents CO2 | 580 to 600 / 900 ppm (was 585 / 1,180 to 1,200) |

| Energy | Home at 50% | Home at 60% |
|---|---|---|
| Air handlers, average | 1,005 W, 24.1 kWh a day (was 999, 24.0) | 461 W, 11.1 kWh a day (was 454, 10.9) |
| Tent humidifiers | 34 W (was 18) | 24 W (was 12) |
| CO2 fans: the room's / the two tents' | idle / 3 W | idle / 3 W |

## 5. What does not balance

- **Energy, both homes, by a lot** (Realistic mode). The air handlers add about
  38 kWh a day to the family home's 12 and about 24 to the solo home's 3.4,
  against 15.1 and 5.76 of supply: about 27 and 16 more panels (with the
  mushrooms' CO2 fans and the harder-working tent humidifiers, 2026-09-27). In
  the Station-supplied mode the solo home closes (about 4.3 kWh a day) and the
  family home no longer does (about 15.8 against 15.1: the mushrooms' fresh air
  costs it about 3.8 kWh a day more than before; one more panel). The cause is
  mostly one number: the greenhouse and the court leak 0.5 air changes an hour
  (an Earth greenhouse's, UGA B792) into the drier home air, about 215 L of
  water a day, and condensing a litre out of 50% air costs about 105 Wh of fan
  against 27 Wh out of the 75% greenhouse. The levers, each an operator's call:
  the home's air at 60% (measured above), a tighter bulkhead between the grow
  rooms and the home, EC-motor fan coils (the catalogue's EC option publishes no
  figures), a smaller greenhouse for one person. The Station-supplied mode is
  the other answer: the station's plant pays.
- **Oxygen, both homes, slightly.** The crops give out one molecule of oxygen
  per molecule of carbon dioxide they fix, but people burning fat and protein
  breathe in more oxygen than the carbon dioxide they breathe out (respiratory
  quotient 0.87), and the leak takes a little. Once the carbon settles the
  family home loses about 0.4 kg of oxygen a day and the solo home about 0.2,
  out of about 2,900 kg in the home's air: years before it matters, but not
  closed. An electrolyser would make it back for about 6.5 kWh per kg (2.6 and
  1.3 kWh a day); neither home carries one yet.
- **The mushroom tents' carbon dioxide.** FIXED on a game's first days
  (2026-09-27, section 7): a CO2 fan in each tent and one for the mushroom room
  hold every tent at 900 ppm and 90%, and stale air now costs a mushroom its
  crop. NOT fixed once the family home's air settles near 675 ppm: the tents'
  humidifiers cannot wet the air their fans must pull, and the tents fall to 71
  to 82% (section 7).
- **Climate.** The coils take the latent heat of every litre they condense,
  2.45 MJ a litre (FAO-56): about 19 kW in the family home and 7 kW in the solo
  home, on top of the sunlight the glass-roofed greenhouse absorbs. It goes to
  the station's radiators, which the game does not model; the rooms sit at a
  fixed 21 C and the home at 20 C.
- **Nitrogen.** The 0.46 kg of air a day that leaks carries about 0.35 kg of
  nitrogen, which a station has to bring in (raw chemistry inputs).

## 6. Found, and left alone

- The station has a well pump and rain catchment (`water_pump`, the cistern's
  rain port). Aboard, neither exists; which the home is, station or ground site,
  is the BUG-090 decision.
- The "outdoor" fields breathe into the weather: aboard, their room
  (`room-fields`) is inside the hull and their water should reach the home's
  air too.
- The family home's Wi-Fi router (0.6 RF, wired and powered) stunts the whole
  garden to death within minutes at full power; the balance test leaves RF
  emitters out.
- FIXED 2026-09-27: `data/home_outline.json`'s sealed air loop quoted a CO2
  sorbent bed at 0.6 kWh a day per person as "ISS CDRA magnitude" and trace
  contaminant control as "about 200 W = 0.2 kWh/day". From Hanford 2004 Table
  7.3.2 (read 2026-09-27; "Carbon Dioxide Removal Assembly 206" peak "1,487",
  operational average "860" W; "Trace Contaminant Control Subsystem 209" peak
  "250", average "180" W): the sorbent bed is 860 W x 24 h / four crew = 5.16
  kWh a crew member at 1.00 kg of CO2, 4.8 kWh at the outline's 0.922 kg (range
  4.0, the machine's 4.35 kWh per kg of capacity, to 5.2); the TCCS is 180 W x
  24 h = 4.3 kWh a day, one unit however few people share the air. The sealed
  total goes from 5.0 + 0.6 + 0.2 = 5.8 to 5.0 + 4.8 + 4.3 = 14.1 kWh a day, the
  sealed bare minimum from 9.8 kWh a day, 24 panels and 8 banks to 18.1, 45 and
  16 (December: (18.1 - 0.7) / 0.39 = 44.6 panels; five overcast days: 45 x 0.12
  + 0.7 = 6.1 kWh a day, a 60 kWh hole), the LED comparison from 12x to 7x (the
  plants' two jobs against the two machines, 69 / 9.8), and the sealed luxury
  tier no longer closes on its own array (63 panels, not 28). Each changed line
  in its balance and assumptions arrays says what it replaced;
  `data/self_sufficiency/cannot_close.ron` still holds (electrolysis alone is
  more than the 4.0 kWh bare minimum home).
- Trace contaminants (the TCCS) are not modelled.

## 7. 2026-09-27: the mushrooms' air, the meter and a short night

### The mushrooms' carbon dioxide, before and after

Measured by the shipped-homes test (full planting, a day to settle, then a day
and a night; the Realistic mode, which only changes who pays).

| | Family: before | Family: after | Solo: before | Solo: after |
|---|---|---|---|---|
| Tents' CO2, ppm | 1,568 to 1,639 | 900 | 1,180 to 1,203 | 900 |
| Tents' humidity | 90.0% | 90.0% | 90.0% | 90.0% |
| Mushroom room CO2, ppm | 1,005 | 600 to 604 | 585 | 580 to 600 |
| Home air CO2, ppm | 438 to 501 | 443 to 509 | 395 to 423 | 395 to 423 |
| CO2 fans (all), W | none | 60.6 (room 50.7, tents 9.9) | none | 3.1 (room idle) |
| Tent humidifiers, W | 26.5 | 122.2 | 17.5 | 33.8 |
| Tent humidifiers' water, L a day | 6.4 | 29.3 | 4.2 | 8.1 |
| Air handlers, W | 1,496 | 1,574 | 999 | 1,005 |
| Mushroom machines, kWh a day | 0.64 | 4.39 | 0.42 | 0.88 |

The fans cost more in humidifier power than in fan power: the fresh air they
pull through a tent is air its humidifier has to wet to 90%, and the room fan
pulls the room's air toward the home's drier 50%. The air handlers work a
little harder too, condensing that water back out of the home.

**Why 900 and 600.** The setpoints are game choices sized on the model's own
balance (`print_the_mushroom_co2_fans_at_other_setpoints`):

| Tents / room, ppm | Family: tents' humidity | Family: fans + humidifiers, W | Solo: tents' humidity | Solo: fans + humidifiers, W |
|---|---|---|---|---|
| 800 / 600 | 80.5 to 88.6% (humidifiers flat out) | 70.6 + 144.0 | 90.0% | 6.2 + 44.9 |
| 800 / 500 | 80.5 to 88.2% (flat out) | 86.0 + 144.0 | 90.0% | 19.0 + 45.2 |
| 900 / 600 (shipped) | 90.0% | 60.6 + 122.2 | 90.0% | 3.1 + 33.8 |
| 900 / 550 | 90.0% | 77.1 + 122.4 | 90.0% | 6.8 + 33.9 |
| 950 / 650 | 90.0% | 41.1 + 105.9 | 90.0% | 2.2 + 29.8 |

Cornell's growers keep oysters below 800 ppm, but at 800 the family home's
tents draw more air than their T3 humidifiers can wet and fall under the
oyster's 85%. 900 leaves 100 ppm under the 1,000 limit and holds 90%; 950 is
cheaper but leaves 50. Holding the room lower than 600 buys nothing at the
tents and runs its fan harder.

**What stale air costs.** `oysters_in_stale_air_lose_health_and_the_co2_fans_spare_them`:
six full racks with no CO2 fans put their tents at about 1,540 ppm, and the
oysters are held to about 86 health at the Realistic setting (93 at Gentle);
with the fans they keep 100.

### What does not balance: the family home once its air settles

A new game's home air starts at 400 ppm and the measured days above sit at 440
to 510. It does not stay there: the family home's crops only take up all the
carbon dioxide the home makes, 5.89 kg a day (the mushrooms 3.56 of it), near
675 ppm. Measured (`print_the_shipped_homes_with_the_home_air_at_its_settled_co2`):
started at 700 ppm its day ran 597 to 689 with the crops taking up 5.77 kg;
started at 800, 651 to 751 and 5.99. (This doc's first pass estimated 800 from
the law; the measurement puts it near 675.) There the mushroom room sits at
693 to 784 ppm with its fan flat out, the tents' fans hold 900 ppm only by
pulling so much of that air through that their humidifiers run flat out
(144 W), and the tents fall to 71 to 82% humidity, under the oyster's 85%
window: at the driest hour the humidity cap (Kim 2013's curve, `humidity.ron`
FUNGI) holds an oyster to about 91 health, so the family home's mushrooms lose
up to about 9% of their crop once its air settles, where the carbon dioxide
alone would have cost them nothing at 900 ppm. The solo home settles near 465
ppm and holds (tents 900 ppm and 90%).

The mushrooms are 60% of the family home's carbon dioxide and cannot fruit in
the air they enrich. The fixes are design calls: a home of its own for the
mushrooms' air (a small scrubber that hands their carbon dioxide to the
greenhouse, where the crops want it), fewer racks, bigger tent humidifiers
(the T7 is 1.3 L an hour at 100 W), or a greenhouse that takes up more.

### The static power meter

`the_static_meters_charge_what_the_machines_draw`. The family home's Usage
meter now reads 79.2 kWh a day Station-supplied and 117.0 Realistic; the old
one read 253.4 in either mode, much of it the work stations' nameplates (the
stove 1.2 kW, the oven 2.2 kW) and the air handlers' (325 W each) for 24 hours.
What is left over the Energy loop's hand-worked 15.8 is the same defect in
machines outside this rung: the family water heater's 2 kW element for 24 hours
(48.0 kWh a day, where the solo home's carries a continuous-equivalent 50 W for
its 1.2 kWh), the washer's 500 W (12.0, where the solo home's carries 12 W for
0.3 kWh a day), and the 33 towers' 15 W pump and light ports (11.9). Each needs
a sourced daily draw (`average_watts`) before the meter and the loop agree.

### Tests (2026-09-27), each seen red on a deliberate break

In `src/systems/farming/life_support_tests.rs`:
`the_air_handlers_keep_their_power_through_a_short_night` (red with the old
0 W shedding, and with only powered handlers writing their draw, in a grow
room's air step or in the home's),
`a_short_island_keeps_the_scrubber_before_an_optional_load` (red with the old
priority order), `a_tent_breathes_only_the_blocks_planted_at_each_species_rate`
(red charging the tent's whole load), `stale_air_costs_a_fruiting_mushroom_its_crop`
(red ignoring the mode), `oysters_in_stale_air_lose_health_and_the_co2_fans_spare_them`
(red leaving the CO2 cap out of the crop's ceiling),
`co2_fans_hold_the_tents_and_their_room_at_their_setpoints` (red leaving the
CO2 fans out of the exchange, and drawing a switched fan at the cube of its
share), `a_co2_controller_lands_its_air_on_the_setpoint`
(red with the once-a-slice band controller), and the shipped-homes test
extended (red with the tent fans moved out of the family home, and with the air
handler's `average_watts` at its nameplate). In `src/systems/electrical.rs`:
`a_short_island_sheds_its_optional_loads_first` and
`a_load_drawing_nothing_is_never_shed`. In `src/machines.rs`:
`the_static_meters_charge_what_the_machines_draw` and
`every_shipped_mushroom_rack_has_a_wired_co2_fan`.
