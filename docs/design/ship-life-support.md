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
respiratory quotient of 1.

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
homes' loops treated the home's air as free.

### Family home (three residents, 6,600 kcal)

| Water, litres a day | |
|---|---|
| Into the air: greenhouse crops / court towers | 541 / 106 |
| Into the air: tents breathed / humidified / household | 7 / 6 / 5 |
| Condensed and returned: greenhouse (2 units) / court (1) / home (4) | 357 / 75 / 234 |
| Total returned, of 666 put in | 666 (0 on walls, 0.01 leaked) |
| Drawn by the irrigation | 757 |
| of which the outdoor fields' (into the weather) / the crops' tissue | 57 / 40 |
| Garden's net draw on the tanks | 91 |

| Air, kg a day (measured day) | |
|---|---|
| Carbon dioxide out: people / mushroom tents | 2.33 / 3.56 |
| Carbon dioxide taken up by the crops | 4.95 (rising with the air's CO2; the law settles it near 800 ppm, when uptake meets 5.9) |
| Oxygen given out by the crops / breathed in (people 1.95, tents 2.59) | 3.57 / 4.54 |
| Home air CO2 over the day | 441 to 503 ppm (scrubber idle) |
| Mushroom room / tents CO2 | about 1,005 / 1,570 to 1,640 ppm |
| Air leaked overboard | 0.46 |

| Energy | Home at 50% | Home at 60% |
|---|---|---|
| Air handlers, average | 1,496 W, 35.9 kWh a day | 924 W, 22.2 kWh a day |
| Tent humidifiers | 26 W | 14 W |
| Exhaust fan, CO2 scrubber | idle, idle | idle, idle |

### Solo home (one resident, 2,200 kcal)

| Water, litres a day | |
|---|---|
| Into the air: greenhouse crops / court towers | 199 / 43 |
| Into the air: tents breathed / humidified / resident | 2 / 4 / 2 |
| Condensed and returned: greenhouse (1) / court (1) / home (3) | 15 / 12 / 224 |
| Total returned, of 250 put in | 250 |
| Drawn by the irrigation | 293 (fields 29, tissue 15) |
| Garden's net draw on the tanks | 43 |

| Air, kg a day | |
|---|---|
| Carbon dioxide out: resident / tents | 0.78 / 1.19 |
| Taken up by the crops | 1.85 at 395 to 425 ppm (settles near 470 ppm) |
| Oxygen given out / breathed in | 1.33 / 1.51 |
| Tents CO2 | 1,180 to 1,200 ppm |

| Energy | Home at 50% | Home at 60% |
|---|---|---|
| Air handlers, average | 999 W, 24.0 kWh a day | 454 W, 10.9 kWh a day |
| Tent humidifiers | 18 W | 12 W |

## 5. What does not balance

- **Energy, both homes, by a lot** (Realistic mode). The air handlers add about
  36 kWh a day to the family home's 12 and about 24 to the solo home's 3.4,
  against 15.1 and 5.76 of supply: about 23 and 16 more panels. The cause is
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
- **The mushroom tents' carbon dioxide.** The tents' fresh air was sized for
  400 ppm intake; their room sits about 540 ppm over the home's (the gap doc's
  estimate, now measured), so every tent runs over the 1,000 ppm the oysters
  fruit under (Lin 2022). The exhaust fans answer humidity, not carbon dioxide;
  a room fan run for CO2, or tents sized to their room's real air, is the fix.
  The model does not yet harm a mushroom for it.
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
- `data/home_outline.json`'s air loop quotes a CO2 sorbent bed at 0.6 kWh a day
  per person as "ISS CDRA magnitude"; Hanford 2004 gives the CDRA 860 W average
  for about four crew, about 5 kWh a day per person. It also gives trace
  contaminant control as "about 200 W = 0.2 kWh/day"; 200 W is 4.8 kWh a day
  (Hanford 2004's ISS TCCS: 180 W average). Not edited here (another task owns
  that file).
- Trace contaminants (the TCCS) are not modelled.
