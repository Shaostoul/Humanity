# Batteries and Storage

A battery bank is what turns power that comes when it likes (the sun at
noon, the wind when it blows) into power you can use when you need it: at
night, on a still grey week, in a power cut. It is also the one part of a
home energy system that holds a large amount of energy in a small box, and
that is where its hazards come from. A flooded lead-acid battery gives off
hydrogen and holds sulfuric acid. A lithium battery that is damaged, charged
wrongly or badly made can overheat and burn in a way that ordinary
firefighting does not stop. This guide is about deciding how you will store
electricity at home and living with it safely:

- what a battery bank does, in the five numbers that describe one: capacity,
  power, depth of discharge, round-trip efficiency and cycle life;
- lead-acid against lithium iron phosphate against the other lithium
  chemistries;
- sizing a bank for one night, and for several days without sun, worked
  through with real numbers;
- charge controllers and inverters, in plain words;
- temperature, and why a lithium battery must not be charged below freezing;
- ventilation and hydrogen for flooded lead-acid batteries;
- thermal runaway and fire for lithium, and where a home battery may go;
- and disposal and recycling.

**Where this guide stops.** Connecting a battery bank to a house's wiring, and
building a bank out of loose cells or car batteries, is work for a licensed
electrician. [Your First Solar Power](/library#first-solar-power) draws the same
line for the same reasons, and covers the plug-together portable power
station that is the safe first rung. The Department of Energy's guide to
small wind systems puts the test plainly for anyone tempted to install
batteries themselves: if you do not know how to handle and install batteries
safely, you should probably hire an installer. This guide teaches you enough
to choose a system, size it, place it and live with it, and to know when
something is wrong.

Units: energy is in kilowatt hours (kWh), the unit on an electricity bill,
and watt hours (Wh; 1,000 Wh is 1 kWh). Power, how fast energy flows right
now, is in watts (W) and kilowatts (kW). Temperatures are in degrees
Fahrenheit with Celsius in brackets.

## What a battery bank does

When electricity goes into a battery it drives a chemical reaction, and the
energy is stored in that reaction; when the battery discharges, the reaction
runs back the other way and current flows out. That is the Department of
Energy's description, and everything below is about how much, how fast, and
how often.

### Capacity and power: the tank and the tap

DOE describes two separate sizes for any energy store. **Energy capacity** is
how much it can hold, in kilowatt hours. **Power capacity** is how fast it can
let that energy out, in kilowatts. They are as different as the size of a
water tank and the width of its tap, and a battery needs to be big enough in
both.

Two real examples, clearly labelled as illustrations of what the numbers look
like, not recommendations:

- The National Laboratory of the Rockies (NLR; until recently the National
  Renewable Energy Laboratory) models a representative home battery in its
  2025 Annual Technology Baseline as a 5 kW, 12.5 kWh system: it can hold 12.5
  kWh and deliver up to 5 kW, so it empties at full power in 2.5 hours.
- Tesla's Powerwall 2 datasheet (North American backup version, 2017) lists
  13.5 kWh of usable energy, 5 kW of continuous power for charging and
  discharging, and 7 kW for up to 10 seconds, for the moment a motor starts.

For scale, the Energy Information Administration puts the average US
residential customer at 10,791 kWh a year in 2022, about 899 kWh a month,
which works out to about 29.6 kWh a day. So a 12.5 kWh home battery holds
about ten hours of an average American home's electricity, not a day of it.
Homes that live on batteries get there by using much less.

### Ampere hours, and how to turn them into kilowatt hours

Many batteries, especially lead-acid ones, are labelled in ampere hours (Ah).
DOE's glossary defines the ampere hour as a quantity of electricity: current
in amperes multiplied by the hours it flows, used as a measure of battery
capacity. To turn it into energy, multiply by the battery's voltage:

**Wh = Ah x volts.**

SimpliPhi's installation manual for its PHI 3.8 lithium iron phosphate
battery gives a worked example of its own: the 48 volt model is rated 75 Ah
at a nominal 51.2 volts, and 51.2 x 75 = 3,840 Wh, the 3.8 kWh in its name.
A 12 volt, 100 Ah battery holds 1,200 Wh, or 1.2 kWh (worked here).

There is a catch with lead-acid. Pacific Northwest National Laboratory
(PNNL) notes in its 2022 energy storage assessment for DOE that most lead-acid
batteries have their ampere hours rated at the 50 to 100 hour rate, and that
drained faster they give less. From a manufacturer's data it tabulated how
much of the rated energy is available: about 61 percent when emptied in 2
hours, about 86 percent in 10 hours, and about 98 percent in 24 hours. A
lead-acid label is a best case for a slow drain.

### Depth of discharge

Depth of discharge (DoD) is how much of the battery's capacity you take out
before charging it again: draining a 10 kWh bank to 7 kWh left is a 30
percent depth of discharge. It matters because how deeply a battery is
cycled decides how long it lasts, and because makers often keep some capacity
back that you never see. The Powerwall 2 datasheet lists a total energy of 14
kWh, a usable energy of 13.5 kWh, and a depth of discharge of 100 percent:
100 percent of the usable part. The other half kilowatt hour is never offered
to you. When you compare batteries, compare usable energy.

### Round-trip efficiency

Round-trip efficiency is the share of the energy you put in that you get back
out. NLR's Annual Technology Baseline defines it as the ratio of useful
energy output to useful energy input, and adopts 85 percent as representative
for home batteries. DOE says plainly that storage is never 100 percent
efficient, because converting energy and getting it back always loses some.

Where the losses sit, from PNNL's 2022 assessment (figures for 2021):

| Chemistry | Battery alone (DC to DC) | Including a two-way inverter (AC to AC) |
|---|---|---|
| Lithium (LFP or NMC) | 89.55 percent | 86 percent |
| Lead-acid, emptied in 2 hours | 77 percent | 74 percent |
| Lead-acid, emptied in 10 hours | 85 percent | 82 percent |
| Lead-acid, emptied in 100 hours | 87 percent | 84 percent |

PNNL assumed an inverter that loses 2 percent each way. The Powerwall 2
datasheet claims more than 90 percent from AC back to AC, at the beginning of
the battery's life, at 77 F (25 C) and 3.3 kW; SimpliPhi's manual lists a 98
percent operating efficiency for its batteries. Makers' figures are measured
in good conditions, which is why a planner uses a figure like NLR's 85.

### Cycle life and calendar life

A battery does not stop working one day; it holds less and less. PNNL counts a
battery's cycle life as the number of charge and discharge cycles until it
holds 80 percent of its rated energy. Its 2022 figures, at 80 percent depth of
discharge:

- lithium iron phosphate (LFP): about 2,400 cycles;
- lithium nickel manganese cobalt oxide (NMC): about 1,520 cycles;
- lead-acid, single cells: about 1,370 cycles, and PNNL notes that 12 volt
  blocks last significantly fewer cycles than single cells.

Batteries also age with time whether they are used or not. PNNL's calendar
lives are 16 years for LFP, 13 years for NMC and 12 years for a lead-acid
system (it cites 14 years for a lead-acid battery held on float at 77 F (25
C), and 5 years for one kept fully charged in uninterruptible power supply
duty). NLR's baseline assumes a home battery lasts 15 years, with its fading
capacity made up by maintenance, and about one cycle a day.

Makers quote much higher numbers. PNNL found LFP makers' data implying 2,475
to 6,250 cycles at 80 percent depth of discharge, and SimpliPhi's manual
claims more than 10,000. PNNL explains the gap: makers' cycle tests run
several cycles a day, which leaves out ageing with time, and extrapolate from
a limited number of real cycles, while most warranties limit the owner to one
full cycle a day. Plan on the independent figures and treat a long warranty
as a bonus.

A consequence worth writing down (worked here): because a battery is counted
as worn out when it holds 80 percent of its rating, a bank sized exactly for
today's need will be about a fifth short near the end of its rated life.

## Lead-acid, lithium iron phosphate, and the other lithium chemistries

**Lead-acid** comes in two families, as DOE's energy storage safety plan
(April 2024) describes them. Vented or flooded batteries have liquid acid you
can see and top up with water; valve regulated batteries (VRLA) are sealed,
either with the acid soaked into a glass mat (AGM) or set as a gel. DOE calls
lead-acid one of the oldest and safest battery technologies, easy to install,
and more than 98 percent recyclable. Its two weaknesses, as DOE puts them,
are its weight and footprint, and its need to be fully recharged regularly
when it is used for daily partial charging and discharging. DOE gives the
ideal working range for lead-acid in daily cycling as 10 to 50 percent depth
of discharge.
PNNL adds that lead-acid degrades faster when it sits for a long time at a
low state of charge (the plates sulfate), and caps its own planning at 80
percent depth of discharge so the battery never stays below 20 percent.

Not every lead-acid battery will do. DOE's small wind guidebook says
deep-cycle batteries, such as golf cart batteries, can discharge and recharge
80 percent of their capacity hundreds of times, but car batteries are
shallow-cycle batteries and should not be used for renewable energy storage,
because they last only a short time when deeply cycled.

**Lithium-ion** is a family, told apart by the chemistry of the positive
electrode. DOE's safety plan describes the two that matter for homes:

- **Lithium iron phosphate (LFP)** has a lower voltage and so stores less
  energy for its size, but it is cheaper, lasts more cycles and is more
  thermally stable, because the bond between phosphorus and oxygen in it is
  stronger than the metal to oxygen bond in most other lithium chemistries.
  That is why grid storage has moved to it. NLR's 2025 baseline bases its home
  battery costs on LFP cells.
- **Nickel manganese cobalt (NMC)** packs more energy into less space, which
  is why electric cars have favoured it, and some home batteries still use it.
  DOE notes that the high-nickel versions can have poorer structural and
  thermal stability, and PNNL notes that for safety NMC is usually charged to
  no more than 90 percent, a limit LFP does not need.

DOE also names what is coming: a manganese-enriched LFP (LMFP) with more
energy for its size, sodium-ion batteries, and solid-state batteries, which
replace the flammable liquid electrolyte and were not yet commercial when DOE
wrote in 2024.

| | Lead-acid (deep-cycle) | Lithium iron phosphate | NMC lithium |
|---|---|---|---|
| Size and weight for the energy | Heaviest (DOE) | Lighter (EPA) | Lightest (DOE) |
| Depth of discharge to plan on | 10 to 50 percent daily (DOE), 80 at most (PNNL) | 80 percent (PNNL's cycle figures) | 80 percent, charged to 90 at most (PNNL) |
| Round trip, battery alone | 77 to 87 percent (PNNL) | 89.55 percent (PNNL) | 89.55 percent (PNNL) |
| Cycles at 80 percent (PNNL) | about 1,370 (single cells) | about 2,400 | about 1,520 |
| Calendar life (PNNL) | 12 years | 16 years | 13 years |
| Upkeep | Watering if flooded; full recharges; ventilation | A battery management system does it | A battery management system does it |
| What goes wrong | Hydrogen, acid, sulfation if left low | Thermal runaway, less readily | Thermal runaway |
| End of life | Over 98 percent recyclable (DOE) | Recycle; never the trash (EPA) | Recycle; its nickel and cobalt are worth more (PNNL) |

**How to choose.** For a bank that is cycled every day, LFP is where the
independent figures point: more cycles, longer calendar life, and better
thermal stability than NMC. Lead-acid still makes sense where it is sized so
large that ordinary days only cycle it shallowly, where its weight does not
matter, and where it can have a ventilated place of its own, and its
recycling is the most established. The worked example below shows how a
lead-acid bank sized for days without sun ends up shallowly cycled on
ordinary nights, which is the way it likes to be used.

## Sizing a bank: a worked example

The numbers in the first step are an illustration, not anybody's measured
household; the method is the point. [Your First Solar Power](/library#first-solar-power)
shows how to add up your own loads from their labels, and the same method
scales to a house.

**Step 1: the load.** Suppose everything you want to run adds up to 4.0 kWh a
day, and 2.4 kWh of that falls between sunset and sunrise (lights, the
refrigerator, the router, a fan).

**Step 2: the inverter's cut.** Household appliances run on alternating
current and a battery stores direct current, so the energy passes through an
inverter, which loses some. NLR's PVWatts calculator assumes an inverter
efficiency of 96 percent at rated power unless told otherwise, so the bank
has to deliver:

2.4 / 0.96 = **2.5 kWh** for the night.

**Step 3: one night's bank.** Divide by the depth of discharge you will
allow.

- Lithium iron phosphate at 80 percent: 2.5 / 0.80 = **about 3.1 kWh**.
- Lead-acid at 50 percent, the deep end of DOE's ideal range: 2.5 / 0.50 =
  **5.0 kWh**.

**Step 4: days without sun.** DOE's small wind guidebook says battery banks
are typically sized to supply the load for 1 to 3 days. Three dark days of the
whole 4.0 kWh load is 12.0 kWh at the appliances, 12.0 / 0.96 = 12.5 kWh from
the bank, and at 80 percent, the deepest either chemistry should go (PNNL's
cap for lead-acid, the cycle-life figure for LFP):

12.5 / 0.80 = **about 15.6 kWh**.

That is five times the one-night lithium bank. It also changes how the bank
lives on an ordinary night: taking 2.5 kWh out of 15.6 is a 16 percent depth
of discharge, inside DOE's ideal range for lead-acid. A lead-acid bank that
has been drained to 80 percent in a dark spell needs a full recharge as soon
as the sun returns, because it degrades while it sits low (PNNL).

**Step 5: allow for age.** A 15.6 kWh bank near the end of its rated life
holds about 80 percent of that, 12.5 kWh: exactly the three days, with nothing
spare (worked from PNNL's definition). Size up, or accept that the third dark
day will get harder over the years.

**Step 6: check the power, not just the energy.** Add up the watts of
everything that could run at the same moment, and remember that motors (a
refrigerator, a pump) draw a surge for a moment as they start. The bank and
its inverter must cover both. For scale, SimpliPhi's 48 volt PHI 3.8 lists 37.5
amps (1.92 kW) continuous and 60 amps (3.07 kW) for 10 minutes, so two in
parallel give about 3.8 kW continuous (worked here); the Powerwall 2 lists 5
kW continuous and 7 kW for 10 seconds.

**Step 7: refilling.** After the dark spell the panels have to put back more
than the loads took out, because of the round-trip loss. At NLR's 85 percent,
replacing the 12.0 kWh the appliances used takes about 12.0 / 0.85 = 14.1 kWh
of charging, on top of that day's own 4.0 kWh (worked here). In winter that
can take days. At this project's home site in Silverdale, Washington, NLR's
PVWatts calculator (version 8.5.0, queried 27 September 2026, a 400 W panel
tilted 40 degrees to the south) gives 440.7 kWh a year at the inverter, about
1.21 kWh a day on average, but only 17.6 kWh in January, about 0.57 a day, a
third of August's 52.5 kWh month (the figures `data/machines/home.ron` also
records). This is why off-grid homes keep a generator as a backstop, which is
the next section.

## Charge controllers and inverters, in plain words

**The charge controller** stands between the panels (or a turbine) and the
battery. DOE's small wind guidebook says a home that is not connected to the
grid needs one to keep the batteries from overcharging. An NLR engineering
paper (2024) describes charge controllers as power electronics that control
the voltage and current going into a battery from a direct current source.
Two kinds are common:

- **PWM** (pulse width modulation) controllers connect the panel to the
  battery through a switch that pulses to control how much energy flows. They
  suit panels whose voltage is close to the battery's, and they are cheaper
  and simpler.
- **MPPT** (maximum power point tracking) controllers convert the panel's
  voltage down to the battery's while holding the panel at the voltage where
  it makes the most power. They cost more at the start and, by being more
  efficient, pay for it over time.

A controller has to be set for the battery's chemistry. DOE's safety plan
names temperature compensation of the charger's output as one of the measures
that made thermal runaway less likely in sealed lead-acid batteries, and
SimpliPhi's manual tells owners to use only a charger approved for its lithium
batteries.

**The battery management system (BMS)** is built into a lithium battery. DOE's
safety plan says it must limit charging currents to safe levels, and it is
where abuse is detected early. One failure it guards against: cells in a
string age at different rates, and if they drift out of balance the fuller
cells can be overcharged. DOE also says plainly that a BMS can do little once
thermal runaway has begun, which is why the rest of this guide still matters.

**The inverter** converts the battery's direct current (DC) into the
alternating current (AC) that household appliances use. DOE explains that it
does this by switching the direction of the DC input back and forth very
rapidly, and filters the result into a clean sine wave. The small wind
guidebook adds that very small systems can run DC appliances straight from
the batteries, that an inverter slightly lowers the system's efficiency, and
that it lets the home be wired for ordinary AC.

**A battery only keeps the lights on in a power cut if the system was built
for it.** DOE's page on inverters explains that the small inverters attached
to household solar systems ride through small disturbances on the grid, but
disconnect and shut down if a disturbance lasts a long time or is larger than
normal. Solar and battery systems can run without the grid in an outage only
if they are designed to do so, with an inverter that can. Ask about this
before you buy, not during the first outage.

**The generator backstop.** For the times when neither the sun nor the wind is
producing, DOE's small wind guidebook describes most off-grid hybrid systems
providing power from batteries, an engine generator, or both: when the
batteries run low, the generator can power the home and recharge them, and
modern controllers can run it automatically. A generator brings its own
hazards; [Heating Water](/library#heating-water) and [Firewood](/library#firewood) cover
carbon monoxide, and CDC's rule is that a generator runs only outside, more
than 20 feet (6 metres) from windows, doors and vents.

## Temperature

**Do not charge a lithium battery below freezing.** The US Fire
Administration's battery page says to store lithium-ion batteries at room
temperature when possible, and not to charge them at temperatures below 32
F (0 C) or above 105 F (40 C). It adds: never leave them in direct sunlight or
in hot cars, which is a fire risk. SimpliPhi's manual explains the cold half
of that from the maker's side: its batteries are not harmed by cold as such,
but charging below freezing can damage their health and cycle life and voids
the warranty; if one must be charged in the cold, it should be at no more than
5 percent of its capacity per hour (a rate of C/20, one that would take 20
hours to fill it).

**Read two temperature ranges on a datasheet, not one.** SimpliPhi lists a
charging range of 32 to 120 F (0 to 49 C) and a wider operating range of minus
4 to 140 F (minus 20 to 60 C): the battery may power a cold cabin that it
cannot safely be charged in. The Powerwall 2 datasheet lists an operating
range of minus 4 to 122 F (minus 20 to 50 C). Where a bank will see frost,
the questions for the seller are what the battery does when it is too cold to
charge, and whether anything keeps it warm enough.

**Lead-acid needs protecting from both ends.** DOE's small wind guidebook says
lead-acid batteries require protection from temperature extremes, and PNNL's
calendar lives are quoted for a battery held at 77 F (25 C).

## Flooded lead-acid: hydrogen, acid and ventilation

A charging lead-acid battery gives off hydrogen and oxygen. DOE's safety plan
calls it charge gas; OSHA's rules call these gassing batteries. A sealed
(VRLA) battery recombines the gas inside the case, and DOE notes that thermal
runaway was first seen in sealed batteries, when that recombination ran out of
control, though such events are usually less severe than in lithium
batteries. A flooded battery vents its gas.

**Why that matters.** DOE's hydrogen safety fact sheet gives hydrogen a wide
flammable range, 4 to 74 percent in air, and a very small ignition energy, 0.02
millijoules. It is odourless, colourless and tasteless, so no human sense will
warn you. It is also much lighter than air, and DOE notes that a hydrogen leak
indoors would briefly collect on the ceiling before moving toward the corners.
So the ventilation that matters is at the top of the room or the battery box,
where the gas gathers.

OSHA's rules for batteries at work are the clearest statement of what a
safe battery room looks like, and they read as well for a garage as for a
workshop. From 29 CFR 1926.441:

> Batteries of the unsealed type shall be located in enclosures with outside vents or in well ventilated rooms and shall be arranged so as to prevent the escape of fumes, gases, or electrolyte spray into other areas.

> Ventilation shall be provided to ensure diffusion of the gases from the battery and to prevent the accumulation of an explosive mixture.

The same section requires face shields, aprons and rubber gloves for anyone
handling acid or batteries, facilities for quickly drenching the eyes and
body within 25 feet (7.62 m) of where batteries are handled, and floors that
resist acid. OSHA's rule for charging industrial truck batteries,
29 CFR 1910.178(g), adds four habits worth keeping at home:

- no smoking where batteries charge;
- no open flames, sparks or electric arcs near them;
- tools and other metal objects kept off the top of uncovered batteries,
  where they can short the terminals;
- and, in OSHA's words for charging batteries: "acid shall be poured into
  water; water shall not be poured into acid."

**What is inside.** The EPA says a lead-acid battery may contain up to 18
pounds of lead and about a gallon of corrosive, lead-contaminated sulfuric
acid. DOE's small wind guidebook sums up the placement rule: for safety,
batteries should be isolated from living areas and electronics, because they
contain corrosive and explosive substances. DOE's safety plan lists proper
watering and spill containment as the safety measures for flooded batteries.

## Lithium: thermal runaway and fire

**What thermal runaway is.** DOE's energy storage safety plan defines it as
an accelerating release of heat inside a cell, from a chain of heat-releasing
reactions, that shows up as an exponential and uncontrollable rise in the
cell's temperature. It can be started by electrical, mechanical or thermal
abuse (overcharging, crushing, overheating) or by a manufacturing defect, and
DOE points out that a lithium-ion battery carries everything a fire needs:
fuel in its liquid electrolyte, oxygen released from the metal oxide
electrode, and heat from the reactions themselves.

**LFP is less prone, not immune.** DOE is direct about it: lithium iron
phosphate is not a silver bullet, incidents have happened in LFP systems too,
and an LFP cell in thermal runaway gives off more hydrogen, which raises the
risk of an explosion.

**Why a battery fire is different.** DOE's safety plan says that a failing
lithium cell vents a flammable mixture of hydrogen, carbon monoxide, carbon
dioxide and organic compounds, and that in an enclosure this gas can quickly
pass the point where it can explode. Putting out the visible flames may not
cool the cells enough to stop the next ones from failing, and a fire that
seems out can reignite. The case DOE cites is McMicken, Arizona, in April 2019,
where four firefighters were injured when a battery enclosure was opened, let
air in, and exploded. The lesson for a household is simple: do not open the
cabinet or cupboard of a battery that is smoking, hissing or hot to see what is
wrong.

**Buy a system that has been tested.** The US Fire Administration's lithium-ion
handout says to look for the mark of a Nationally Recognized Testing
Laboratory on the packaging and the product, and to check the Consumer Product
Safety Commission's site (saferproducts.gov) for recalls first. For home
battery systems the standard is UL 9540, which UL Solutions describes as
covering energy storage systems and their equipment, drawing on UL 1973 for
the batteries and UL 1741 for the inverters. A separate test, UL 9540A, is
the method for measuring whether a thermal runaway fire spreads through a
battery system, and it is the one the fire code cites. The Powerwall 2
datasheet, for example, lists UL 9540, UL 1973 and UL 1741 among its safety
listings.

**Where a home battery may go.** The fire code for this is NFPA 855. It is a
copyrighted standard, so what follows is NFPA's own public summary of its
residential chapter (a 2021 NFPA blog); your local building department decides
which edition applies where you live, and may add to it.

- It covers home battery units of 1 to 20 kWh each. Bigger units, or more
  energy in one area than the limits below, fall under the rules for
  commercial installations.
- The total allowed is 40 kWh in utility closets and storage or utility
  spaces, and 80 kWh in garages and detached structures, on exterior walls,
  and outdoors.
- Those are the only places allowed: attached and detached garages; outside,
  on an exterior wall or on the ground, at least 3 feet (914 mm) from doors and
  windows; utility closets; and storage or utility spaces. Not bedrooms, not
  living spaces.
- In an unfinished room, the walls and ceiling must be covered with at least
  5/8 inch (16 mm) gypsum board.
- A battery where a vehicle could hit it needs a barrier such as bollards, or
  should be moved or mounted out of reach.
- A home with a battery system needs interconnected smoke alarms throughout,
  including in the garage or room that holds the battery; where a smoke alarm
  cannot go, such as an attached garage, a heat detector connected to the
  alarms.

**Warning signs, and what to do.** The US Fire Administration's list: stop
using a lithium-ion battery if you notice an odour, a change in colour, too
much heat, a change in shape, leaking, or odd noises. Its advice after a flood
adds that a battery giving off a cloud of smoke or gas, or popping or hissing,
means move to a safe distance and call the fire department. Its one-line rule
for a battery fire is: get out quickly. Get everyone out, call the fire
department once you are at a safe distance, and do not go back in; DOE's
account above is why nobody should open the battery's cabinet to look.

**After a flood.** The US Fire Administration asks owners to tell the fire
department if lithium-ion batteries in a home or garage may have been
flooded, to disconnect them from chargers, to move flooded batteries and
vehicles out of buildings (a vehicle 50 feet from anything that burns), and to
keep flood-damaged battery products in metal or non-combustible tubs at least
6 feet from debris and buildings. They must not go out with storm debris.

## Disposal and recycling

**Lead-acid.** The EPA's instruction is to return it to a battery retailer or
a household hazardous waste collection programme, and never to put it in the
trash or a recycling bin. The industry that takes them back is mature: PNNL
describes recycling of car, marine, industrial and forklift batteries as an
established trade with set return routes, and DOE puts lead-acid at more than
98 percent recyclable.

**Lithium.** The EPA's rule is short: lithium-ion batteries should not go in
household garbage or recycling bins. Take them to a battery recycler or a
household hazardous waste collection point, and to prevent fires, tape their
terminals or put each one in its own plastic bag. The EPA adds that even used
batteries can hold enough energy to injure or start fires, and that a damaged
battery calls for the maker's own handling instructions. A crushed battery in
a garbage truck or a sorting plant can start a fire; the US Fire
Administration's handout says the same about trash and recycle bins.

**A whole home battery** is too big and too complex for its owner to remove.
The EPA's advice is to contact the maker of the energy storage equipment or
the company that installed it.

## Warning signs

- **A lithium battery gives off an odour, changes colour or shape, leaks, gets
  too hot, or makes odd noises.** Stop using it (US Fire Administration). If it
  is smoking, hissing or popping, get everyone out and call the fire
  department from a safe distance. Do not open its cabinet (DOE).
- **A lithium battery is about to be charged somewhere below 32 F (0 C).** Do
  not, unless its maker says it is protected against charging in the cold (US
  Fire Administration, SimpliPhi).
- **A flooded lead-acid battery's space has no outside vent or airflow, or its
  vents are blocked.** Do not charge there, and keep every flame, spark and
  cigarette away (OSHA). Hydrogen has no smell, so a room that smells fine
  proves nothing (DOE).
- **Acid on the floor, or a cracked or bulging case.** Put on a face shield,
  an apron and rubber gloves before going near it (OSHA). OSHA's battery room
  rules also require the means to flush away and neutralise spilled acid to be
  on hand.
- **A bank that runs out sooner than it used to.** It may be ageing (PNNL
  counts a battery worn out at 80 percent of its rating), or its cells may have
  drifted out of balance, which DOE warns can overcharge the fuller cells. Have
  it checked.
- **Anything wired into the house that trips breakers, sparks, buzzes or
  smells hot.** That is a job for the installer or a licensed electrician, not
  the owner (the boundary at the top of this guide).

## How the game models it

The homes in this project run on real battery banks, and since 27 September
2026 those banks carry the night.

- **The battery.** A bank is a `Battery` component
  (`src/ecs/components.rs`) with four numbers: the energy it holds, its
  capacity, and the fastest it can charge and discharge. Both shipped homes use
  the same bank, 4 kWh with 2 kW each way (`data/machines/home.ron` and
  `home_solo.ron`): the family home has eight of them (32 kWh), the one-person
  home two (8 kWh). Capacity and power are separate numbers in the game, as they
  are in a real battery.
- **The night.** Every tick, `src/systems/electrical.rs` works out each wired
  circuit on its own: its supply is what its generators make plus what each of
  its banks can give, up to that bank's discharge limit and the energy it
  holds. Loads are fed most important first, and what the supply cannot carry
  is switched off, the optional loads before the critical ones. A bank drains
  only for the loads it actually kept on, and charges from real surplus, up to
  its charge limit and its capacity. The tests `batteries_carry_the_night_load`
  and `a_bank_at_its_limit_sheds_the_optional_loads_and_pays_only_for_the_rest`
  hold it to that. Energy moves on the game clock, so a night slept through at
  high speed costs the batteries the whole night.
- **The backstop.** A home's fuelled generator starts only when its circuit's
  free sources fall short of the load, its batteries are below a quarter full,
  and its fuel drum holds fuel; an empty drum means no power. That is the
  hybrid system DOE's small wind guidebook describes.
- **What you see.** The Homes page's Live power card shows the batteries'
  charge as a percentage and in kWh, and roughly how many hours it would run
  the present load with no generation. The Construction editor's Buildability
  check asks whether the batteries can carry the night: the home's average load
  less its steady generators, for every hour outside the 4.5 representative sun
  hours it assumes.
- **The meter.** The Construction page's power meter credits each solar panel
  with what it makes at the home's site less a 90 percent battery round trip,
  from the Powerwall 2 datasheet, because an off-grid home sends most of its
  solar through its batteries. [Ship Life Support](https://github.com/Shaostoul/Humanity/blob/main/docs/design/ship-life-support.md)
  (section 7) records the figures and why neither home's energy budget closes
  yet.

What the game simplifies, so you do not learn it from the game: the live
batteries lose nothing, so a bank gives back every watt hour put into it,
where a real lithium bank returns about 85 to 90 percent and lead-acid less;
a bank can be drained to
empty every night with no harm, where a real one is planned at 80 percent
(lithium) or 50 (lead-acid); there is no temperature, so nothing stops a
bank charging in the cold; batteries never age or lose capacity; there is no
chemistry, so no lead-acid or lithium to choose between, no hydrogen, no acid,
and no thermal runaway or fire; there are no charge controllers or inverters,
and their losses are not charged anywhere in the live power; the Buildability
check counts a bank's whole capacity as usable; and a bank's charge is not
saved, so it starts half full each time the home is loaded. The
[Sim Realism Roadmap](https://github.com/Shaostoul/Humanity/blob/main/docs/design/sim-realism-roadmap.md) (row 16) keeps the
list of what is left to make the battery real.

## Sources

Grouped by what kind of authority each one is. United States government
publications are public domain; the rest are copyrighted, so every fact
taken from them is restated here in our own words.

### United States government (public domain)

- US Department of Energy, Office of Electricity. Energy Storage Safety
  Strategic Plan, April 2024 (lead-acid's two families, its safety, its
  recycling, the 10 to 50 percent depth of discharge and full recharges;
  lithium-ion chemistries, thermal runaway, LFP's stability and its limits,
  vent gas, McMicken, reignition, the battery management system and cell
  balancing; charge gas and temperature-compensated charging).
  https://www.energy.gov/sites/default/files/2024-05/EED_2827_FIG_SafetyStrategy%20240505v2.pdf
- US Department of Energy, WINDExchange. Small Wind Guidebook, page updated 26
  August 2026 (stand-alone systems need a charge controller; deep-cycle
  against car batteries; DC loads and inverters; isolating batteries from
  living areas; protecting lead-acid from temperature extremes; banks sized
  for 1 to 3 days; the hybrid generator backstop; the ampere hour; hiring an
  installer). https://www.energy.gov/cmei/systems/windexchange/small-wind-guidebook
- US Department of Energy. Solar Integration: Solar Energy and Storage Basics
  (how a battery stores energy; energy capacity and power capacity; storage is
  never fully efficient), and Solar Integration: Inverters and Grid Services
  Basics (what an inverter does; riding through and disconnecting; running
  without the grid only if designed to), both modified 31 March 2025.
  https://www.energy.gov/eere/solar/solar-integration-solar-energy-and-storage-basics
  and https://www.energy.gov/eere/solar/solar-integration-inverters-and-grid-services-basics
- US Department of Energy, Office of Energy Efficiency and Renewable Energy.
  Hydrogen Safety fact sheet (flammable from 4 to 74 percent in air, 0.02 mJ
  to ignite, odourless and colourless, collects at the ceiling indoors).
  Undated. https://www1.eere.energy.gov/hydrogenandfuelcells/pdfs/h2_safety_fsheet.pdf
- US Energy Information Administration. How much electricity does an
  American home use? (10,791 kWh in 2022, about 899 a month), last updated 8
  January 2024. https://www.eia.gov/tools/faqs/faq.php?id=97&t=3
- Occupational Safety and Health Administration. 29 CFR 1926.441, Batteries
  and battery charging (unsealed batteries vented, ventilation against an
  explosive mixture, protective equipment, eyewash within 25 feet),
  https://www.osha.gov/laws-regs/regulations/standardnumber/1926/1926.441 ;
  and 29 CFR 1910.178(g), Changing and charging storage batteries (no
  smoking, no flames or sparks, metal kept off batteries, acid into water),
  https://www.osha.gov/laws-regs/regulations/standardnumber/1910/1910.178 .
  Both read 27 September 2026.
- US Fire Administration. Battery fire safety (store at room temperature, do
  not charge below 32 F or above 105 F, warning signs, get out quickly), page
  last reviewed 7 November 2024,
  https://www.usfa.fema.gov/prevention/home-fires/prevent-fires/batteries/ ;
  Lithium-Ion Battery Safety for Electronic Devices handout (testing
  laboratory marks, recalls, disposal),
  https://www.usfa.fema.gov/downloads/pdf/publications/lithium-ion-battery-safety-handout.pdf ;
  and Lithium-Ion Battery Safety After Flooding handout,
  https://www.usfa.fema.gov/downloads/pdf/publications/lithium-ion-battery-after-flooding-handout.pdf .
  The handouts are undated; read 27 September 2026.
- US Environmental Protection Agency. Used Household Batteries (lead-acid:
  up to 18 pounds of lead and a gallon of acid, where to return it; large
  lithium-ion systems: contact the maker or installer), last updated 21 July
  2026, https://www.epa.gov/recycle/used-household-batteries ; and Used
  Lithium-Ion Batteries (smaller and lighter than other batteries for the same
  energy; not in the garbage or recycling bin, tape the terminals, used
  batteries can still start fires), last updated 20 March
  2026, https://www.epa.gov/recycle/used-lithium-ion-batteries
- Centers for Disease Control and Prevention. Carbon Monoxide Poisoning
  Basics (a generator only outside, more than 20 feet from windows, doors and
  vents), reviewed 12 January 2026. The CDC server refused a direct download
  on the day of reading, so the sentence was read through a web fetch and
  matched against the independently checked [Heating Water](/library#heating-water).
  https://www.cdc.gov/carbon-monoxide/about/index.html

### US national laboratories (licence unconfirmed; cited as the authority, restated here in our own words)

- Viswanathan, V., Mongird, K., Franks, R., Li, X., Sprenkle, V. (Pacific
  Northwest National Laboratory) and Baxter, R. (Mustang Prairie Energy). 2022
  Grid Energy Storage Technology Cost and Performance Assessment, publication
  PNNL-33283, for the US Department of Energy, August 2022 (round-trip
  efficiencies, Tables 4.4 and 4.6; cycle life at 80 percent depth of
  discharge, Table 4.3 and section 4.2.4.2; calendar lives; lead-acid
  available energy by discharge time, Table 4.5; sulfation and the 80 percent
  cap; NMC's 90 percent charge limit; recycling).
  https://www.pnnl.gov/sites/default/files/media/file/ESGC%20Cost%20Performance%20Report%202022%20PNNL-33283.pdf
- National Laboratory of the Rockies (formerly the National Renewable Energy
  Laboratory). Annual Technology Baseline 2025, Residential Battery Storage (5
  kW and 12.5 kWh, LFP cells, 15 year life, about one cycle a day, 85 percent
  round trip). https://atb.nlr.gov/electricity/2025/residential_battery_storage
- National Laboratory of the Rockies. PVWatts API version 8 documentation
  (inverter efficiency, default 96 percent at rated power),
  https://developer.nlr.gov/docs/solar/pvwatts/v8/ ; and a PVWatts 8.5.0
  query made for this guide on 27 September 2026 (latitude 47.645, longitude
  minus 122.695, 0.4 kW, fixed open rack, tilt 40, azimuth 180, 14.08 percent
  losses, weather NSRDB PSM V3 GOES tmy-2020: 440.68 kWh a year, 17.60 in
  January, 52.55 in August), the same query `data/machines/home.ron` records.
- Schnabel, A. and McGilton, B. A Comparison of Battery Charge Controller
  Technologies for Wave Energy Converters, preprint, NREL/CP-5700-90527,
  October 2024 (what a charge controller is; PWM against MPPT). Copyright
  IEEE. https://docs.nlr.gov/docs/fy25osti/90527.pdf

### Standards bodies (copyrighted; cited as the authority, restated here in our own words)

- O'Connor, B. Residential Energy Storage System Regulations. National Fire
  Protection Association blog, 1 October 2021 (NFPA 855's residential
  chapter: unit sizes, energy limits by location, permitted locations,
  gypsum board, vehicle barriers, alarms).
  https://www.nfpa.org/news-blogs-and-articles/blogs/2021/10/01/residential-energy-storage-system-regulations
- UL Solutions. Energy Storage System Testing and Certification (what UL
  9540 covers, UL 1973 and UL 1741 within it, UL 9540A cited by NFPA 855).
  Undated; read 27 September 2026.
  https://www.ul.com/services/energy-storage-system-testing-and-certification

### Manufacturers' documents (copyrighted; illustrative specifications only, restated here)

- Tesla. Powerwall 2 AC datasheet, North America, backup version, dated
  2017-03-21 in its footer (14 kWh total, 13.5 kWh usable, 5 kW continuous and
  7 kW for 10 seconds, 100 percent depth of discharge, more than 90 percent
  round trip at the beginning of life, minus 20 to 50 C operating, UL 9540).
  Read through a copy hosted by a solar retailer, because Tesla's own server
  refused the request.
  https://solarforward.com/pdf/products/Powerwall/Powerwall_2_AC_Backup_Datasheet_EN_NA.pdf
- SimpliPhi Power. PHI 3.8, PHI 2.9, PHI 1.4 and PHI 730 Battery Models,
  installation manual, revision 073019 (the 51.2 volt, 75 Ah, 3.8 kWh
  example; continuous and 10 minute ratings; charging 0 to 49 C and operating
  minus 20 to 60 C; charging below freezing; the maker's cycle claim; use its
  approved charger). Read through a copy hosted by a retailer.
  https://www.solarelectricsupply.com/media/sparsh/product_attachment/custom/upload/Simpliphi-Power-PHI-batteries-installation-manual.pdf

### Inside this project

- `src/systems/electrical.rs`: the power balance, load shedding, the
  batteries carrying the night, and the backstop generator.
- `src/ecs/components.rs` (`Battery`), and the battery banks in
  `data/machines/home.ron` and `data/machines/home_solo.ron`.
- `src/machines.rs`: the Buildability night check and the power meter;
  `src/gui/pages/homes.rs`: the Live power card.
- [Ship Life Support](https://github.com/Shaostoul/Humanity/blob/main/docs/design/ship-life-support.md), section 7: the
  meter's supply side and the homes' energy budgets.
- [Your First Solar Power](/library#first-solar-power), [Heating Water](/library#heating-water)
  and [Firewood](/library#firewood), for the neighbouring skills.
