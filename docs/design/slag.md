# Slag: every use aboard the fleet, and why mining's waste is an endless sink

**Status: research and design pass, 2026-11-13. No code yet.** Every number below is cited to the
dated finding [2026-11-13-slag.md](../reference/findings/2026-11-13-slag.md) (F1 to F28), to the
helper notes in [research/2026-11-13-slag/](../reference/research/2026-11-13-slag/README.md) (A, B
and C entries), or to another named document, or it is marked an **estimate**. Computed numbers
come from [design-arithmetic.js](../reference/research/2026-11-13-slag/design-arithmetic.js)
(`node design-arithmetic.js`). Anything marked **Proposal** is this pass's suggestion, not a
decision.

The operator, 2026-11-13 (master plan 1.5, item 8): "Mining technically already exists. Players
send out drones to mine." And: "We'll have to figure out the various ways we can use the slag.
That way we can justify it as an infinite resource sink." On the mass that armour adds: "Should be
a player vote per ship".

Related: [mothership-master-plan.md](mothership-master-plan.md) (1.5 item 8 the shield and the
vote, 3.14 thrust only to manoeuvre, 3.16 the fleet), [drone-mining.md](drone-mining.md) (the ore
loop), [closed-loop-test.md](closed-loop-test.md) (a closed loop fed only rock powder), the
[active-shielding research](../reference/research/2026-11-13-active-shielding/README.md) (rock and
water per kilogram) and the [mothership sizes research](../reference/research/2026-11-13-mothership-sizes/README.md).

## 0. In short

- **What slag is.** When the smelter melts the metal out of asteroid ore, the stony glass left
  behind is slag. Ours is mostly **magnesium silicate**, close to the dark, heavy rock of Earth's mantle, not the
  lime-rich slag of an Earth steelworks (F1, F7). Rock that is crushed but never melted is called
  tailings; both serve the uses below.
- **How much.** A tonne of iron from a stony asteroid leaves about 2 to 3.5 tonnes of slag, or up
  to 24 tonnes of rock if only the metal grains are picked out; a carbonaceous asteroid about the
  same; a metallic asteroid only 10 to 15 kg (computed from F1 to F4). Baking water out of
  carbonaceous rock leaves 4 to 19 tonnes of dry rock per tonne of water (computed), the biggest
  waste stream of all.
- **Where it goes, biggest first:** armour packed on the hull outside the water; reaction mass a
  mass driver throws overboard; the drums' own bulk (balance weights, blocks, pavers, fill); the
  farms' soil; oxygen and light metals pulled out of it by electrolysis; and small uses
  (phosphorus filters, a heat store, insulation, art).
- **Is it endless?** Yes, in two honest senses. **At a player's scale:** one drone trip makes 5 to
  30 kg of slag, and one tonne per square metre of armour over Drum 1 alone is 3.5 million tonnes,
  118 to 706 million trips (computed). **At the fleet's scale:** NASA's 1975 settlement design used
  slag as its shield and found "the colony's shielding requirements exceed the slag production"
  (F22); each new 1 km drum's shell takes 14.5 million tonnes per t/m2, and a mass driver can throw
  slag away usefully forever. The one caveat: if the shield's water is baked out of carbonaceous
  rock, the rock left over outruns the hull (section 3).
- **The vote.** Each ship's residents choose how deep the slag armour is beyond the water. Before
  voting they see the mass it adds, how much slower the ship turns, the propellant each manoeuvre
  costs, the yearly dose and the meteoroid hits it stops: 1 t/m2 over Drum 1 adds about 7% to the
  mass, 4% to the turn time, and cuts 5 mSv a year to about 3.3 to 4.0 (computed, section 4.3).
- **First rungs:** slag delivered to the hull with a line in the fleet ledger and the armour gauge;
  then the vote; then the smelter's tap choices (quench, cool, cast) with blocks and pavers; then
  rock dust for the soil.

## 1. What slag is

**In plain words.** Ore is metal mixed with rock. Melt it, and the metal sinks while the rock
floats on top as a glassy crust: that crust is slag (F5). On Earth, steelworks add limestone so
the crust flows, which is why Earth's slag is about 40% lime. Asteroid rock has almost no lime, so
its slag is mostly silica and magnesia, the ingredients of olivine and pyroxene, the green and dark
minerals of the heavy rock of Earth's mantle (A17, "close to an ultramafic rock"). Cooled fast in water it becomes glass grains; cooled slowly it
becomes stone (F7).

**What asteroid slag is made of** (wt%, after the iron is taken out; A17, computed from F1):

| | Stony asteroid (H chondrite) | Carbonaceous (CI) | Earth blast-furnace slag (F7) |
|---|---|---|---|
| Silica, SiO2 | 55.6 | 53.2 | 36 |
| Magnesia, MgO | 35.3 | 36.4 | 12 |
| Alumina, Al2O3 | 3.2 | 3.7 | 10 |
| Lime, CaO | 2.6 | 2.9 | 39 |
| Lime to silica | about 0.05 | about 0.05 | about 1.1 |

If the iron in the minerals is not reduced, iron oxide stays in the slag: about 10% of a stony
rock and 22% of a C1 rock (F2). Chromium oxide comes to about 0.8% (A17), which matters for soil
(section 2.5). Metallic asteroids leave only troilite, phosphide and a few silicate grains (A14).

**How much per tonne of iron-group metal** (A14, computed from F1 to F4):

| Ore | Metal grains only | All iron reduced | Notes |
|---|---|---|---|
| Stony, H type | 4.9 t of rock | 2.3 t of slag | Ross's round "about 20%" metal gives 4.0 t (F2) |
| Stony, L type | 11 t | 3.0 t | |
| Stony, LL type | 24 t | 3.5 t | the leanest metal, but the richest in precious metals (F4) |
| Carbonaceous, CI and CM | about 1,000 t (0.1% free iron) | 2.2 to 2.4 t | water (up to 17%) and carbon (2 to 4%) leave as gas first |
| Stony-iron (mesosiderite) | about 1 t | | about half metal (F4) |
| Metallic (iron) | 10 to 15 kg | | about 99% metal (F2) |
| Earth's blast furnace, for comparison | | 0.25 to 0.30 t | ore 60 to 66% iron (F6) |

**The bigger stream: rock left after water and carbon.** A carbonaceous rock holds up to 16.6%
water (CI) or 10.3% (CM), and C-types generally 5 to 20% (A2, A7). Baking it out leaves 4.8 t of dry
rock per tonne of water from CI rock, 8.5 t from CM, and 4 to 19 t across the range (computed).
Drum 1's shield is 35 million tonnes of water (space-program.js); taken from CI or CM rock, it would
leave 169 to 299 million tonnes of rock (computed). Water from ice-rich bodies leaves little.

**Precious metals follow the metal, not the slag.** About 33 ppm of platinum-group metals and gold
sit in H-chondrite metal and 72 ppm in LL metal (A26), so a well-separated slag carries almost none.
"Mining the slag for trace metals" is therefore a small use; the metal product is where the value is.

**The current game** already makes slag: `smelt_iron` turns 2 iron ore and 1 coal into 1 iron ingot
and 1 slag, `smelt_copper` leaves 6 slag (data/recipes.csv, with worldsteel's and Gorai's ratios in
its comments), `slag_0` is a 1 kg item (data/items.csv), `make_concrete_slag` uses it in place of
half the gravel, and `tests/byproduct_use_lint.rs` keeps every byproduct used (docs/FEATURES.md,
"Byproducts and Machine Output to Storage"). The drones' ore comes from C, S and M asteroids
(data/asteroids/belt.ron).

## 2. Every use

**The whole list**, biggest sink first. Energy is per tonne of slag; Drum 1 numbers use its 3.53 km2
hull and 7,781 people (`node scripts/space-program.js`, 2026-11-13).

| # | Use | What it takes | How much it absorbs | Kind of sink | Source |
|---|---|---|---|---|---|
| 1 | **Armour on the hull**, outside the water (radiation and meteoroids) | crush or granulate and pack, about 2 kWh/t (F12); cast tiles for the outer face, 0.36 to 0.4 MWh/t from cold (F13) | **3.5 Mt per t/m2** over Drum 1; 47 Mt if slag replaced all the water (13.4 t/m2); 14.5 Mt per t/m2 per 1 km drum; 7.2 x 10^12 t per t/m2 for a fleet of ten billion | **endless while the fleet grows** | F19, F20, F22; computed |
| 2 | **Reaction mass** for a mass driver | thrown as powder at about 8 km/s: 11.9 MWh per tonne thrown (F21) | 6,040 t per m/s of Drum 1; a 10 MW driver throws about 7,400 t a year (1.2 m/s a year) | **endless over time**, limited by power and the manoeuvre budget | F21; computed |
| 3 | **Spin balance**: fixed trim and winched counterweights | cast or bagged blocks | 1 to 2% of the spinning mass: 130,000 to 260,000 t for Drum 1, once | one-time per drum | F23 (the 1 to 2% is for spacecraft; applying it to a drum is an estimate) |
| 4 | **Structure**: fill, aggregate, cast blocks, tiles, pavers, linings, sintered blocks | aggregate about 2 kWh/t (F12); casting 0.36 to 0.4 MWh/t from cold, little more than holding heat if poured from the smelter (estimate); sintering at 1,000 to 1,100 °C, energy not found (F14) | the stores layer under the floor, walls, paths and roads; the floor's soil and buildings weigh about 2.6 t/m2 (sizes research, judgement), up to 8.2 Mt over Drum 1, part of it slag | one-time per drum; grows with the fleet | F7, F12 to F14 |
| 5 | **Soil**: rock dust as the soil's mineral part and its yearly top-up | milling; cement fineness costs 30 to 40 kWh/t (F10), rock dust is coarser (less, estimate) | once 0.22 to 0.36 t/m2 at 0.3 m deep (0.68 to 1.12 Mt if all Drum 1's floor were soil, an upper bound); then 0.1 to 4 kg/m2 a year (300 to 12,600 t a year over that floor) | one-time, then small yearly | F25 |
| 6 | **Oxygen and metals** by molten electrolysis | 7.5 to 47 MWh per tonne of slag (computed from F8) | a 10 MW cell takes 1,900 to 11,700 t a year; gives oxygen, iron, silicon, aluminium, magnesium | endless over time, limited by power | F8 |
| 7 | **Phosphorus filters** on the waste-water line | crushed slag in a bed | 205 to 284 kg a person a year, 1,600 to 2,200 t a year for Drum 1 (estimate), then into the soil (that step unsourced) | recurring, then joins use 5 | F27; closed-loop-test.md 6.6 |
| 8 | **Heat store** | a packed bed of crushed slag heated to 750 °C | about 770 t per 100 MWh stored | one-time, small | F24 |
| 9 | **Insulation** (slag wool) | melt and spin (energy not cited; at least a melt's 0.36 to 0.4 MWh/t, estimate) | 5 to 8 kg per m2 of wall or ceiling | one-time, small | F15 |
| 10 | **Binders**: alkali-activated slag cement | grind 30 to 40 kWh/t (F10) plus 55 to 80 kg of soda (Na2O) per tonne of binder (F11) | per tonne of concrete; bounded by sodium | bounded | F9 to F11 |
| 11 | **Fibre** (continuous basalt-type) | needs added silica: our slag's acidity is about 1.55, fibre needs 3 to 8.5; 3.0 to 3.5 kWh/kg (F16) | little | not a sink | F16 |
| 12 | **Art and decoration** | cast slag, slag glass colourant | a token | token | F17 |

**Not uses:** clear glass (it needs about 72% silica, 14% soda and almost no iron, F18; windows come
from purified regolith silica, not slag); carbon capture (slag binds 81 to 211 kg of CO2 a tonne,
F28, but carbon is scarce aboard and binding it starves the farms: an emergency scrubber only);
clothing (basalt fibre is not for clothing, sizes research section 5).

### 2.1 Armour on the hull

**In plain words.** The ship already wraps each drum in a non-spinning shell of water, 10 t/m2, for
5 mSv a year (master plan 1.5 item 8). Slag packed outside the water does two jobs: it thins the
cosmic rays further, and it takes every meteoroid strike before the water compartments do. Rock
outside, water inside, so the hydrogen-rich water soaks up the neutrons the rock makes
(active-shielding research, a judgement there, not sourced).

**Radiation.** Rock needs about 1.5 to 2.1 times water's mass for the same dose (F19). Beyond the
water, each t/m2 of slag multiplies the dose by about 0.65 to 0.79 (**estimate**: the low end
extends the regolith dose curve's deepest slope, the high end converts slag to water at the 2.1
ratio; one OLTARIS run of the real stack would settle it). So 1 t/m2 takes 5 mSv a year to about 3.3
to 4.0; 3 t/m2 to about 1.4 to 2.5 (computed).

**Meteoroids.** By the Grün model (F20), Drum 1's shell meets about **2,100 meteoroids of a
milligram or more a year** and one of a gram or more every four years (computed). A gram digs about
7 cm into processed regolith and 10 g about 15 cm (computed from F20's crater rule). So a slag face
of about 15 cm (0.3 t/m2) catches every strike up to about 10 g. Without it, each milligram-class
strike lands on a compartment wall; whether it pierces depends on the wall (**estimate**). A radiation
shield of a few t/m2 already makes meteoroids a minor matter for the hull as a whole (F20, SP-413):
the slag's meteoroid job is protecting the valved compartments, so the ship rarely drains one.

**How it is packed (Proposal).** Crushed or granulated slag in bags held in a mesh outside the
water compartments, about 2 kWh/t (F12); cast tiles of 0.36 to 0.4 MWh/t (F13) only at the outer
face, where strength helps. Steel slag carrying free lime or magnesia swells when it meets water
(F7); ours has enough silica that its magnesia should bind into pyroxene rather than stay free (an
**estimate** from its composition near enstatite, MgSiO3), but slag next to the water compartments
is still quenched to glass or kept dry until a test says otherwise.

**The structure's share.** At the 0.05 g manoeuvring cap (master plan 3.14) each t/m2 adds
0.49 kN/m2 to the shell's supports (computed). Kalpana One warns that "Contact between these is a
catastrophic failure mode" between a spinning hull and a nearby still shield (F23): the shell's
position control must hold its gap with the armour's mass included.

**Enough slag?** Drum 1's 4.84 Mt of steel (1.37 t/m2, active-shielding research), smelted from
stony ore, leaves 10.9 to 23.7 Mt of slag or rock (computed): **enough to armour the drum 3.1 to 6.7
t/m2 deep from its own steel.** An all-slag shield (13.4 t/m2) would take 47 Mt, more than its steel
made, which is SP-413's finding again (F22).

### 2.2 Reaction mass

**In plain words.** A mass driver is a long magnetic sling: it throws a pinch of crushed slag off
the ship at about 8 km/s, and the ship moves the other way. NASA studied it in the 1970s (F21). It
is slow (about 190 N for every MW of power) but its fuel is waste.

- **What it costs:** 11.9 MWh of electricity per tonne thrown (computed from F21). Moving Drum 1 by
  1 m/s takes 6,040 t of slag and 72 GWh, 36 days of all of Drum 1's 84 MW (computed; 84 MW from
  master plan 1.5 item 6). A 10 MW driver gives Drum 1 about 1.2 m/s a year.
- **What it saves:** a steam thruster burning the shield's water (192 s, C11) would spend 25,600 t of
  water for the same 1 m/s (computed): slag thrown instead keeps the shield whole.
- **So it is the station-keeping and trim engine**, not the manoeuvring drive; the drive itself is
  master plan 3.14's.
- **Powder, never pellets:** solid pellets are "a hazard to other spacecraft" (F21); loose powder
  spreads harmlessly. Thrown in a star system, the powder stays there when the fleet jumps.
- **Spinning up a drum** needs no slag: the non-spinning shell is the reaction mass (C12, SP-413).

### 2.3 Structure: fill, blocks, pavers, linings

**In plain words.** On Earth most slag becomes road stone and concrete (F6). Aboard, it becomes the
fill under the floors, the blocks of walls, the pavers of paths, and the wear linings of the ore
chutes.

- **The smelter's tap decides the product** (F7): quench it in water or air to glass grains (the
  binder grade), let it cool slowly in beds to stone (aggregate and fill), or pour it into moulds
  (cast blocks, tiles, linings). Using it while molten saves the 0.36 to 0.4 MWh/t of melting it
  again (F13).
- **Cast slag** is strong in compression (cast basalt 550 MPa) and weak in tension (36 MPa), and
  brittle, so joints are the design problem (F13). Fused cast basalt pipe linings outlast 30 years
  (F13): the right lining for the drones' ore chutes and hoppers.
- **Sintered and printed blocks** range from gypsum-weak to 345 MPa depending on how fully they
  fuse; vacuum (free aboard, outside) raises strength (F14).
- **The road home (Proposal):** master plan 1.3's "consistent yellow brick road" home is paved with
  cast slag pavers, so every path in the drum is mining's work underfoot.
- **The stores layer under the floor** (active-shielding research) is slag-filled where it is not
  stores, counting toward the shield once a transport calculation confirms it (1.5 item 8).

### 2.4 Spin balance

**In plain words.** A spinning drum wobbles when its weight is uneven, and people, goods and water
move all day. Fixed slag blocks set where the layout is light balance it once (SP-413 asks the
layout to "balance masses around the rim", C15); weights on winched cables, as Kalpana One proposes
(F23), or water pumped between rim tanks (C16) take the moving part.

- Passive balancers need no more than 1 to 2% of a spacecraft's mass (F23); applied to Drum 1's
  13 Mt of spinning steel, soil and buildings that is 130,000 to 260,000 t (computed, an
  **estimate** since the 1 to 2% is for spacecraft).
- **Proposal:** slag blocks are the fixed trim and the winched counterweights; pumped water is the
  fast, continuous trim. The balance accuracy a 250 m drum needs is not sourced (finding, unknowns).

### 2.5 Soil

**In plain words.** The closed loop is fed only rock powder (closed-loop-test.md; master plan 1.3).
Milled slag is that powder: it gives magnesium, silicon, some calcium and iron, but almost no
potassium or phosphorus, and no nitrogen or carbon at all (E1 to E3 in the sizes research's closed-
loop notes).

- **Building soil:** soil is about 45% mineral by volume (F25); 0.3 m of it holds 0.22 to 0.36 t/m2
  of mineral (computed). Kalpana One makes its shield "double as soil" (C13).
- **Topping it up:** 1 to 20 t/ha in field trials (0.1 to 2 kg/m2) and 40 t/ha a year in enhanced
  weathering models (4 kg/m2, F25).
- **Caution (realistic mode):** asteroid slag is magnesium-heavy and about 0.8% chromium oxide (A17);
  the steel-slag fertiliser studies (F26) are lime-rich analogues that did not report chromium. So a
  batch is tested for chromium, nickel and vanadium before it goes into food soil (the finding's
  unknowns say what would settle the safe level).

### 2.6 Phosphorus filters

Phosphorus is the closed loop's hardest leak: about 0.9 kg a person a year is lost at 80% recovery
(closed-loop-test.md 6.6). A bed of crushed slag on the waste-water line holds 2.15 g of phosphorus
per kg (F27). Catching the 0.44 to 0.61 kg a person excretes a year takes 205 to 284 kg of slag a
person, 1,600 to 2,200 t a year for Drum 1 (computed, an **estimate**), after which the loaded slag
would go to the soil as phosphorus-rich rock dust. That last step is unsourced (finding, unknowns),
so it is a Proposal to verify, not a rule.

### 2.7 Heat

- **The smelter's own heat:** a tonne of molten slag gives up about 1.8 GJ (0.5 MWh) cooling from
  1,500 °C (F24); dry granulation by air recovers it and needs no quench water (C20). That heat warms
  the greenhouse or charges the heat store.
- **A heat store:** a packed bed of crushed rock at 750 °C holds about 0.13 MWh a tonne (F24), so a
  100 MWh store for the reactors' daily lamp-load swing takes about 770 t (computed).
- **Not room-temperature buffering:** water holds about 5.5 times more heat per kilogram near room
  temperature (F24), and the shell already holds 35 Mt of it.

### 2.8 Insulation and glass

Slag wool is real and common: about 70% slag in commercial slag wool, about 80% of North American
mineral wool from iron-ore slag, insulating as well as glass fibre (F15). It takes 5 to 8 kg per m2
of wall or ceiling (F15), a small sink. Our slag is more silica-rich than Earth's blast-furnace slag
(the table in section 1), which may need less added rock to fiberise; that is an **estimate**, since
the melt's viscosity is unknown (finding, unknowns). Foamed glass tiles need waste glass as most of
the batch (B22). Clear glass is not made from slag (F18).

### 2.9 Binders

Our slag fails the European cement rule's lime-plus-magnesia test (about 0.68 against more than 1,
F9 computed), and a Portland blend needs limestone the asteroids lack (F11). The alkali route
needs no Portland cement: glassy slag ground fine and mixed with soda (sodium hydroxide, `lye_0`,
and sodium silicate), 5.5 to 8% of the binder (F11). A lime-poor slag still reached 60 MPa this way
(F11). It is bounded by sodium, and no study read covers magnesia as high as ours (finding,
unknowns).

### 2.10 Oxygen and metals out of slag

Molten electrolysis runs a current through melted rock at about 1,600 to 1,700 °C and splits it into
oxygen at one electrode and metal (iron, then silicon and others) at the other (F8, A21). Measured
today at about 104 kWh per kg of oxygen, a mature plant modelled at 50 to 125 kWh/kg (F8); slag that
has lost its iron should do better (F8). Per tonne of slag that is 7.5 to 47 MWh (computed), so a
10 MW cell eats 1,900 to 11,700 t a year: a power sink that turns waste into the ship's aluminium
for craft and silicon for electronics, a late rung.

### 2.11 What Earth does with its slag

In 2017 the USA sold 16.0 Mt: road bases took about 41 to 48%, concrete and asphalt most of the
rest, and glassy slag went almost entirely into cement (F6). Aboard, road stone becomes shielding
fill and pavers, and cement gives way to the alkali route.

### 2.12 Which uses are genuine sinks

1. **Armour for the growing fleet.** Without limit while the fleet adds drums; NASA's slag-shielded
   design ran out of slag (F22). Biggest by far.
2. **Reaction mass.** Without limit over time; how much a year is set by power and how much the
   ship keeps station.
3. **Oxygen and metals.** Without limit over time; set by power.
4. **Structure and balance.** Large, but one-time per drum (they grow with the fleet).
5. **Soil and phosphorus filters.** One-time fill, then a small yearly stream.
6. **Heat store, insulation, binders, art.** Small and bounded.

## 3. Is it endless? The mass balance

**At a player's scale, yes, by orders of magnitude.** A full drone hold (10 units, data/mining/
drone.ron) of iron ore makes 5 kg of slag and of copper ore 30 kg (data/recipes.csv). One t/m2 over
Drum 1 is 3.5 Mt: 118 to 706 million full trips (computed). Ten thousand players flying ten trips a
day would need 3 to 19 years for that one layer on one drum, and a ship of the fleet has 246 drums
(master plan 3.16).

**At the fleet's scale, yes while the fleet grows.** Smelting a drum's own steel leaves enough to
armour it 3.1 to 6.7 t/m2 deep (section 2.1); the rest of the fleet's metal (craft, machines,
cables) adds more; every new 1 km drum's shell takes 14.5 Mt per t/m2. The mass driver and the
electrolysis cell take slag without limit, at a rate set by power.

**Per sector and per fleet** (computed; a sector of 500 is a fifteenth of Drum 1 at its 7,781
people, about 404 m2 of drum floor a person; the soil figures treat all that floor as soil, so they
are upper bounds; a billion people's fleet is 50,000 drums of 1 km, master plan 3.16):

| Use | One sector of 500 | A fleet of a billion |
|---|---|---|
| Armour, per t/m2 (once) | about 227,000 t (its share of Drum 1's hull) | 7.2 x 10^11 t |
| Balance weights (once) | 8,000 to 17,000 t | grows with every drum |
| Soil, building it (once) | 44,000 to 72,000 t | 8.7 x 10^10 to 1.4 x 10^11 t |
| Soil, topping it up (a year) | 20 to 800 t | 4 x 10^7 to 1.6 x 10^9 t |
| Phosphorus filters (a year) | 103 to 142 t | 2.1 to 2.8 x 10^8 t |
| Reaction mass (a year) | a ship's, not a sector's: about 7,400 t per 10 MW of mass driver | the same per ship, times about 204 ships |

**The one real surplus:** water baked out of carbonaceous rock. If Drum 1's 35 Mt of shield water
came from CI or CM rock, it would leave 169 to 299 Mt of dry rock (computed), several times what the
hull's armour, the soil and the structure take. Three honest answers, for the operator to choose
(question 5): take the water from ice-rich bodies where possible; leave the surplus rock at the
asteroid (real mining concentrates ore at the mine and leaves the waste there); or let the armour
vote absorb it (a deep armour, heavy ship).

## 4. The game

### 4.1 The loops

1. **Armour (decided, 1.5 item 8):** drones bring ore to the bow's ore intake (drone-mining.md
   section 8) → the smelter → slag → the **slag intake** beside the ore intake → the hull packer
   adds it to the ship's armour → the dose falls, meteoroid drains stop, the ship grows heavier →
   the residents' vote sets how deep.
2. **Build:** the smelter's tap → glass grains, stone or casts → blocks, pavers, linings,
   counterweights, fill → homes, paths, the road home, the drum's balance.
3. **Grow:** slag → the mill → rock dust → the soil → crops; waste water → the phosphorus filter →
   loaded slag → the soil.
4. **Keep station (later):** slag → the mass driver → the ship holds its place beside a body
   without spending shield water.
5. **Refine (later):** slag plus power → the electrolysis cell → oxygen, iron, silicon, aluminium,
   magnesium.

### 4.2 What a player sees and does

- **At the smelter:** the slag pours out with each ingot as today. In realistic mode the player
  picks the tap: quench (grains), cool (stone) or cast (a mould: block, paver, tile); in simplified
  mode it is plain `slag_0`.
- **At the slag intake:** "Deliver slag to the hull" takes slag from the home's storage (the Barn,
  where machine output already goes); the fleet ledger credits a `slag` line in tonnes, the way it
  credits meals and oxygen (data/ship/fleet_ledger.ron).
- **On the ship's page:** the hull as panels over each sector, tinted by armour depth; the dose
  this year; meteoroid strikes caught by the slag and compartments drained; the ship's mass, turn
  time and propellant per manoeuvre.
- **On the Governance page:** the armour vote with its numbers (4.3).
- **Around the home:** pavers, block walls, the soil's top-up, a sculpture cast in a park.

### 4.3 The vote per ship

**What a resident votes on:** the armour depth beyond the water for their own ship, chosen from
steps in data. The default, before any vote, is no slag armour: the decided 10 t/m2 of water and
5 mSv a year.

**The numbers shown before the vote** (Drum 1 alone; mass of about 48 Mt from the shield, the steel
and the floor's soil and buildings, an **estimate** that leaves out the spine and end works; the
game computes each ship's own):

| Armour beyond the water | Added mass | Ship mass | Turn time (same thrusters) | Propellant per manoeuvre | Dose a year | Meteoroids up to about 10 g caught |
|---|---|---|---|---|---|---|
| none (default) | 0 | 48 Mt | as now | as now | 5 mSv | none: each strikes a compartment |
| 1 t/m2 | 3.5 Mt | +7% | +4% | +7% | 3.3 to 4.0 mSv | all (about 2,100 a year) |
| 2 t/m2 | 7.1 Mt | +15% | +7% | +15% | 2.1 to 3.1 mSv | all |
| 3 t/m2 | 10.6 Mt | +22% | +10% | +22% | 1.4 to 2.5 mSv | all |
| 5 t/m2 | 17.6 Mt | +37% | +17% | +37% | 0.6 to 1.5 mSv | all |

How each column is computed (design-arithmetic.js): the turn time grows with the square root of the
mass for the same thrusters (the slag spreads along the ship like the shell does, an
approximation); propellant for a push grows with the mass; the dose uses the 0.65 to 0.79 factor of
section 2.1. The master plan's own estimates agree (slag on top of the full water adding about a
third, turns about 5% slower for 9% more mass: 1.5 item 8). The panel shows each number with a
one-line reason ("heavier ships turn slower: about 4% slower here"), the way graphics settings say
what they cost (CLAUDE.md).

**The rules (Proposal):**
- A ship-scoped signed proposal on the Governance page (proposals already carry a scope;
  docs/FEATURES.md, "Native Governance page"); only residents of that ship vote; a simple majority
  of votes cast.
- Re-voted at most once a game month, so the hull is not rebuilt weekly.
- **Under unlimited fleet supply** (today's `fleet_supply_mode`, decided 2026-10-04): the voted depth
  applies at once, and each player's deliveries are credited in the ledger, so contributors are in
  the black. **Under limited supply:** the depth grows from deliveries toward the voted target, and
  the gauge shows how far it has come.
- Lowering the depth sends the removed slag to the mass driver's store or to the next drum, never
  overboard as pellets (F21).

**What agility buys** is the operator's to settle (question 1): the vote is only a real choice if a
heavy ship pays for its weight in play.

### 4.4 The data rows

Infinite-of-x (CLAUDE.md): every slag kind, product, recipe and vote step is a row, never code.

| File | Rows (Proposal) |
|---|---|
| `data/chemistry/slags.csv` (new) | one row per slag kind (stony H, L, LL; carbonaceous CI, CM; metallic inclusions; Earth blast-furnace for reference): oxide wt%, density, specific heat (758 J/kg K, F24), the lime-to-silica and acidity ratios, source |
| `data/items.csv` | `slag_0` (exists); `slag_granules_0` (quenched glass), `rock_dust_0` (milled slag or tailings for the soil), `cast_slag_block_0`, `slag_paver_0`, `cast_slag_tile_0`, `slag_wool_0`, `phosphorus_slag_0` (a spent filter bed) |
| `data/recipes.csv` | `quench_slag`, `mill_slag`, `cast_slag_block`, `cast_slag_paver`, `cast_slag_tile`, `spin_slag_wool`, `bind_slag_alkali` (slag grains + `lye_0` + water), `filter_phosphorus`, `electrolyse_slag` (slag + power → oxygen + iron + silicon + `aluminum_ingot_0`), each with its `power_required_watts` from section 2 |
| `data/machines/home.ron`, `data/machines/ship.ron` | the slag mill, the casting bed, the wool spinner; on the ship the slag intake and hull packer (beside the ore intake in data/ship/space_program.toml), the balance winches, the heat store, the mass driver, the electrolysis cell |
| `data/materials.csv` | `cast_slag` (2.9 t/m3, 550 MPa compressive, F13), `slag_aggregate` (up to 1.94 t/m3, F7), `slag_wool` (about 0.04 W/m K, F15) |
| `data/ship/space_program.toml` | per ship: `armour_slag_t_m2` (the vote's result), the slag-to-water dose factors (0.65 to 0.79), so the planning sheet prints mass and dose |
| `data/ship/fleet_ledger.ron` | a `slag` good, in tonnes contributed |
| `data/ship/armour_vote.toml` (new) | the vote's steps (0, 1, 2, 3, 5 t/m2) and the columns the panel shows |
| `data/asteroids/` | per class, the residue per tonne of metal (section 1) for realistic mode |

### 4.5 Both modes (CLAUDE.md, dual modes)

| | Realistic | Simplified |
|---|---|---|
| Slag | composition by ore class; residue per tonne of metal as section 1 (stony ore leaves 2.3 t or more) | one `slag_0` item at today's ratios |
| The tap | quench, cool or cast, each feeding its own uses | automatic |
| Uses | each checks its chemistry: binders need glassy slag, fibre needs added silica, a swelling test before slag goes into a confined wall | every use takes slag one for one |
| Soil | a metals test before food soil | none |
| Energy | each process draws its power from the ship's grid | none |
| Armour | dose from the curves, meteoroid hits from the Grün model, mass and turn time | a bar ("Hull armour: light, standard, heavy") and the same vote with three presets |

### 4.6 Budgets

Slag is counters, not simulated grains: one stock per home, one armour depth per hull panel (a few
hundred panels a drum), updated when slag is delivered. The hull's tint is a per-panel value read by
the existing ship silhouette (master plan 3.16, L3). The network carries a ship's armour depth and
stock at a low rate. Per frame: nothing measurable (estimate). The Dev page's "turn off all reuse"
switch is not involved: nothing here is cached.

### 4.7 The rungs, in order

1. **Slag to the hull.** The slag intake and hull packer (data rows), the `slag` ledger line, the
   armour gauge per ship and per panel, `armour_slag_t_m2` in the planning sheet with mass and dose.
   Proof: a delivered tonne moves the gauge and the ledger; a rig reads both.
2. **The vote.** A ship-scoped proposal type with the numbers panel; the result writes the target.
   Proof: a test proposal changes `armour_slag_t_m2` and the printed mass, turn time and dose.
3. **The smelter's tap and the casts.** Quench, cool, cast; blocks, pavers, tiles, linings; pavers
   on the road home.
4. **Rock dust for the soil.** `mill_slag`, the soil top-up, the metals test (realistic mode), and
   the phosphorus filter loop.
5. **Ore by asteroid class.** Slag compositions and realistic residue per class; the dry rock left
   after water and carbon from carbonaceous rock.
6. **Balance and heat.** Fixed trim and winched counterweights on the drum's balance board; the
   smelter's heat recovered; the heat store for the lamp-load swing.
7. **Slag wool and the alkali binder.** Insulation and cement with their sodium budget.
8. **The mass driver.** Station-keeping on slag powder, its power draw, the powder rule.
9. **Electrolysis.** Oxygen and light metals from slag, a late power sink.
10. **Art.** Cast sculpture and slag glass for the parks.

## 5. Questions for the operator

1. **What does agility buy in play?** Without a cost for weight the vote has one answer.
   **Recommendation:** the turn time and propellant for docking, station-keeping and moving between
   mining fields within a system; dodging a forecast debris or meteoroid swarm (a heavier ship needs
   longer warning); and question 2.
2. **Should the mothership's jump charge grow with its mass**, as the drone's does
   (drone-mining.md section 2)? **Recommendation:** yes. The cruise is a jump (master plan 3.14), so
   without this, mass costs only small manoeuvres; with it, a heavily armoured ship waits longer
   between jumps, a trade every resident understands.
3. **The vote's rules:** who votes, how often, which steps? **Recommendation:** that ship's residents,
   a signed ship-scoped proposal, a simple majority of votes cast, at most once a game month, steps
   0, 1, 2, 3 and 5 t/m2 from data, default 0.
4. **Under today's unlimited fleet supply, does the voted armour appear at once?**
   **Recommendation:** yes, with every delivery credited in the ledger; under limited supply it grows
   from deliveries toward the target.
5. **Where does surplus rock go** if the shield's water comes from carbonaceous rock (4 to 19 t of
   dry rock per tonne of water)? **Recommendation:** leave it at the asteroid, as real mining does,
   and bring home what the ship can use (the armour target, the soil, the structure, the mass driver's
   store); prefer ice-rich bodies for water.
6. **Should stony ore leave realistic slag** (2.3 to 24 t per tonne of metal) in realistic mode?
   **Recommendation:** yes, by asteroid class in data; simplified mode keeps today's ratios.
7. **Slag outside the water only, or also in place of some water** (to free water for the next
   drum)? **Recommendation:** outside only for now, as decided; the water stays at 10 t/m2 until the
   transport calculation of 1.5 item 8, and the vote is about extra depth.
8. **Should the armour show on the outside?** Packed slag makes the shell look like a dark, rough
   rock rather than smooth tanks. **Recommendation:** yes: each hull panel darkens and roughens as
   its armour thickens, so a ship's vote and its residents' mining are visible from a passing craft
   (the per-panel tint of 4.6 costs nothing measurable).

## 6. Sources

- The dated finding with the key figures and their quotations:
  [2026-11-13-slag.md](../reference/findings/2026-11-13-slag.md).
- The three helper notes (88 entries), the residue script and the design arithmetic:
  [research/2026-11-13-slag/](../reference/research/2026-11-13-slag/README.md).
- Shielding by material and the dose curves:
  [2026-11-13-active-shielding](../reference/research/2026-11-13-active-shielding/README.md) and its
  [finding](../reference/findings/2026-11-13-active-shielding.md).
- Rock powder in the closed loop: the sizes research's
  [closed-loop notes](../reference/research/2026-11-13-mothership-sizes/E-closed-loop-and-wind.md).
