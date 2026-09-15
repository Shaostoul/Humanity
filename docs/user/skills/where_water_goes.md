# Where Water Goes

Rain hits your roof. Somewhere above your head there is a slope, and that
slope sends the water to one edge rather than another. It runs into a
gutter somebody chose the size of, down a downspout somebody put on one
corner instead of another, and out onto ground somebody paved, planted,
compacted or left alone.

None of that was decided by the weather. Every part of it was a choice,
most of them made by people who are not you and are not here. And the
water does not stop at your property line. It keeps going, and at some
point it arrives somewhere that matters to somebody else.

That is the whole subject. This guide is about the second half of the
water story: not where your water comes from, but where it goes after it
lands on you, and why that makes you somebody else's upstream.

[Where Water Comes From](where_water_comes_from.md) is the other half. It
covers the sources a person actually has, what is wrong with each, and how
to choose between them. Its most useful habit is to look upstream of any
water you intend to drink. This guide is the same map read in the other
direction, and the uncomfortable part of that is that you are somebody's
upstream whether you meant to be or not.

## The unit that actually matters is the watershed

Property lines are a legal fiction as far as water is concerned. The real
unit is the watershed, and it is worth knowing the definition precisely.

USGS: "A watershed is an area of land that drains all the streams and
rainfall to a common outlet such as the outflow of a reservoir, mouth of a
bay, or any point along a stream channel."

Two things follow from that definition, and both are more useful than they
look.

**A watershed is defined by its outlet, so there are as many watersheds as
there are points on a stream.** USGS again: "It all depends on the outflow
point; all of the land that drains water to the outflow point is the
watershed for that outflow location." Watersheds "can be as small as a
footprint or large enough to encompass all the land that drains water into
rivers that drain into Chesapeake Bay." Your gutter has a watershed. So
does the Mississippi.

**Watersheds nest.** "Larger watersheds contain many smaller watersheds."
Your yard sits inside a creek's watershed, which sits inside a bay's, which
sits inside an ocean basin's. The boundary between two of them is a ridge:
"Ridges and hills that separate two watersheds are called the drainage
divide."

That is why the question "what is upstream of me" has a clean answer and
the question "who is downstream of me" has a longer one. Upstream is a
finite area you can draw. Downstream is a chain that ends in an ocean.

### How to find yours, concretely

The United States has already drawn every one of these boundaries for you,
and the product is free.

It is called the Watershed Boundary Dataset, run by USGS, and it is "a
seamless, national hydrologic unit dataset." Its units "represent the area
of the landscape that drains to a portion of the stream network." The
boundaries are drawn on the ground rather than on a map of who governs
what: they are "determined based on topographic, hydrologic, and other
relevant landscape characteristics without regard for administrative,
political, or jurisdictional boundaries."

Each unit has a number called a hydrologic unit code, or HUC, and the
numbering scheme is the part worth understanding, because once you get it
you can read a HUC the way you read a postcode.

USGS: "Hydrologic unit codes (HUC) are developed using a progressive
two-digit system where each successively smaller areal unit is identified
by adding two digits to the identifying code the smaller unit is nested
within." The dataset "contains eight levels of progressive hydrologic units
identified by unique 2- to 16-digit codes," and it "is complete for the
United States to the 12-digit hydrologic unit."

So a HUC is a nesting doll written as a number. Two digits is a region the
size of the Pacific Northwest. Twelve digits is a piece of ground you could
walk across in a morning. Every extra pair of digits is one step down into
a smaller drainage, and every code contains all the codes above it as its
opening digits.

Here is the real chain for Silverdale, Washington, which is the place this
project is built on. Read it as a zoom.

| Level | Code | Name | Area |
|---|---|---|---|
| 2 digit region | 17 | Pacific Northwest | the whole northwest corner of the country |
| 8 digit subbasin | 17110019 | Puget Sound | 6,370 km2 |
| 10 digit watershed | 1711001907 | Olalla Valley-Frontal Puget Sound | 747 km2 |
| 12 digit subwatershed | 171100190706 | Barker Creek-Frontal Dyes Inlet | 92.4 km2, or 22,828 acres |

Notice that the twelve digit unit is still 92 square kilometres. That is
the finest resolution the national dataset guarantees, and it is nowhere
near the size of a yard. The Silverdale town centre, the lower Clear Creek
valley and the upper Clear Creek headwaters all fall inside that single
unit. Chico Creek, only a few kilometres south, is a different one:
171100190705, Chico Creek-Frontal Sinclair Inlet, 62.4 km2.

The HUC gets you to the right neighbourhood. The last few hundred metres,
you do with your own eyes and a slope.

### The other end of the chain

A HUC12 record carries a pointer to the unit it drains into, so you can
follow your own water outward step by step. Doing that from Silverdale
gives this:

1. **171100190706, Barker Creek-Frontal Dyes Inlet.** The local creeks
   discharge into Dyes Inlet.
2. **171100191000, Port Orchard.** Dyes Inlet leaves south through Port
   Washington Narrows into Sinclair Inlet and Port Orchard.
3. **171100191200, Puget Sound.** The main basin.
4. **171100191300, Rosario Strait-Strait of Juan de Fuca.**
5. **171100200700, Discovery Bay-Strait of Juan de Fuca.**

Then the open North Pacific.

That is five named steps between a downspout in Silverdale and the ocean,
and not one of them is a river. On a peninsula the chain runs through tidal
basins instead, which matters more than it sounds: a basin exchanges water
with the next one slowly and on the tide, so what you put in does not get
flushed away the way it would in a river. It sits in the neighbourhood for
a while.

For most of the country, the easiest starting point is EPA's How's My
Waterway, which "was designed to provide the general public with
information about the condition of their local waters" and reports, for a
place you name, water quality in your local watershed, local drinking water
information, and identified issues including impairments and discharge
violations. Start there, then go to the Watershed Boundary Dataset when you
want the actual boundary and the code.

## Three things can happen to the water that lands on you

Everything in this guide is a consequence of one split. Water that reaches
the ground goes back up, goes down, or goes sideways.

**Up is evaporation and transpiration.** Evaporation is the process that
changes liquid water to water vapour. Transpiration is the release of water
vapour from plant leaves. Plants move a lot more of it than people expect:
USGS puts an acre of corn at about 3,000 to 4,000 gallons, or 11,400 to
15,100 litres, of water given off each day, and a large oak tree at about
40,000 gallons, or 151,000 litres, a year.

**Down is infiltration.** Water soaks into the soil. Some of it stays in
the shallow layer and moves slowly through it; some goes deeper and
recharges an aquifer. This is the path that refills wells and, between
storms, keeps creeks running.

**Sideways is runoff.** Water flows over the surface downhill, into a
ditch, a storm drain or a creek. It is the fastest path, it is the only one
of the three that carries things with it, and it is the one your paving
creates.

The split between them is not even. USGS: "Only about a third of the
precipitation that falls over land runs off into streams and rivers and is
returned to the oceans. The other two-thirds is evaporated, transpired, or
soaks (infiltrates) into the soil."

Hold on to that ratio, because the entire drainage problem of a developed
lot is that the ratio has been inverted on it.

### What decides the split

Five things, and you control more of them than you think.

**How hard it rains.** USGS is direct about the ranking: "The greatest
factor controlling infiltration is the amount and characteristics
(intensity, duration, etc.) of precipitation." A soil that soaks up a
gentle all-day drizzle will shed a cloudburst of the same total volume,
because infiltration has a rate limit and rainfall does not.

**Whether the ground is already full.** "Soil already saturated from
previous rainfall can't absorb much more water, thus more rainfall will
become surface runoff." This is why the tenth wet day of a Pacific
Northwest November behaves nothing like the first.

**The soil itself.** "Some soils, such as clays, absorb less water at a
slower rate than sandy soils." Numbers for this are in the next section.

**The slope.** "Water falling on steeply-sloped land runs off quicker and
infiltrates less than water falling on flat land."

**What is growing on it, or not.** "Vegetation can slow the movement of
runoff, allowing more time for it to seep into the ground." And the one
that dominates everything else: "Impervious surfaces, such as parking lots,
roads, and developments, act as a 'fast lane' for rainfall, whisking it
away into storm sewers."

USGS names the same mechanism when describing what development does: more
of the natural, vegetated landscape is replaced by impervious surfaces such
as roads, which reduce infiltration of water into the ground and accelerate
runoff.

### Real infiltration numbers, by soil

You can get an actual infiltration figure for your own ground, free,
because USDA already assigned one to every mapped soil in the country. The
system is called the hydrologic soil group, it runs A through D, and it is
in the same soil survey that
[Your Soil, Specifically](your_soil_specifically.md) teaches you to read.

The NRCS National Engineering Handbook defines the four groups by the
saturated hydraulic conductivity of the least transmissive layer in the
profile, which is a precise way of saying: the slowest layer sets the pace,
no matter how fast the layers above it are.

| Group | Runoff potential | Conductivity of the slowest layer | Typical textures |
|---|---|---|---|
| **A** | Low | above 40.0 micrometres per second (5.67 inches per hour) | gravel and sand; less than 10 percent clay, more than 90 percent sand or gravel |
| **B** | Moderately low | 10.0 to 40.0 micrometres per second (1.42 to 5.67 in/h) | loamy sand and sandy loam; 10 to 20 percent clay |
| **C** | Moderately high | 1.0 to 10.0 micrometres per second (0.14 to 1.42 in/h) | loam, silt loam, sandy clay loam, clay loam, silty clay loam; 20 to 40 percent clay |
| **D** | High | 1.0 micrometres per second or less (0.14 in/h or less) | clayey textures; more than 40 percent clay, less than 50 percent sand |

The column is simplified. Each threshold in the handbook comes paired with
a depth condition: for groups A, B and C the figure above is for the least
transmissive layer between the surface and 50 cm, and each group has a
second, lower threshold for soils deeper than 100 cm to any restriction.
The group D figure is the one for soils with an impermeable layer between
50 and 100 cm. The full matrix is in the chapter, which is free.

Even simplified, that is a forty-fold span between the top and the bottom
of the table, and it is why the same storm floods one street and not the
next one over.

Two details in the definitions matter more than the numbers.

**Depth counts as much as texture.** Group A and B soils must have "the
depth to any water impermeable layer greater than 50 centimeters" and a
water table deeper than 60 centimetres. And the handbook is blunt about the
other end: "All soils with a depth to a water impermeable layer less than 50
centimeters and all soils with a water table within 60 centimeters of the
surface are in this group," meaning group D, the high runoff class. A
perfectly sandy soil with a hardpan a foot down behaves like clay, because
after the first foot fills there is nowhere left for water to go.

**An impermeable layer is not quite impermeable.** The handbook puts the
conductivity of "an impermeable or nearly impermeable layer" at anywhere
"from essentially 0 micrometers per second (0 inches per hour) to 0.9
micrometers per second (0.1 inches per hour)." Water does cross a hardpan.
Put that 0.1 inches per hour against the 1.42 to 5.67 inches per hour of a
group B soil sitting on top of it and you get the shape of the problem: the
mantle can deliver water to the pan roughly fourteen to fifty-seven times
faster than the pan can take it. The surplus has to go somewhere, and
sideways is the only direction left.

Silverdale is the worked example again, and it is an unusually clean one.
About 64 percent of the mapped land here is a thin gravelly mantle lying on
dense glacial till. Alderwood alone is 49.9 percent of it, and Alderwood is
hydrologic group B with a seasonal perched water table as shallow as 65 cm
and its dense till at a representative 56 cm, mapped anywhere from 50 to
102 cm.

Read that against the handbook criteria and you will see how narrow the
margin is. Alderwood stays in group B, the second-best class, because its
restrictive layer is 56 cm down and the threshold is 50 cm. Six
centimetres. On the parts of the map unit where the till comes up to 50 cm,
it is at the line.

The neighbours make the point from both sides. Indianola, 10.3 percent of
the land, is deep sand with no restriction at all: hydrologic group A, very
high conductivity. Kitsap silt loam, 4.2 percent, is group C. Norma is
B/D, meaning it would drain like a B if it were drained at all, and in
practice has an apparent water table at 0 cm, which is to say at the
surface, and ponds in winter and spring. McKenna is group D outright.

Five soils, four hydrologic groups, all of them within a few miles of each
other. This is why "what does the ground do with rain here" has no regional
answer, only a parcel answer.

## Your roof is the part of this you built

Here is the arithmetic that makes the rest of the guide concrete, and it is
arithmetic rather than a citation, because it is just geometry: one
millimetre of rain falling on one square metre of surface is exactly one
litre of water.

Take a modest house with a 150 square metre roof footprint. Silverdale's
rainfall, depending on which figure you use, is about 1,192 mm a year
averaged across Kitsap County, or 1,446 mm at the Bremerton station 8.5 km
away, which is the number the
[Silverdale gazetteer](../locale/silverdale_wa.md) uses. So:

- **Across a year:** between 179,000 and 217,000 litres come off that roof.
- **In December alone**, at the Bremerton normal of 252 mm: 37,800 litres.
- **In July**, at 21 mm: 3,150 litres. The wet season delivers twelve times
  the dry season.
- **In one ordinary 25 mm storm:** 3,750 litres off the roof, plus another
  1,000 off a 40 square metre driveway.

Nearly five cubic metres of water, from one house, in one afternoon, that
would have gone into the ground if the house were not there.

That is what an impervious surface does. EPA puts it plainly: these
surfaces "do not allow rain and snow melt to soak into the ground which
greatly increases the volume and velocity of stormwater runoff." Both words
matter. Volume, because none of it infiltrates and none of it is
transpired. Velocity, because a smooth hard surface offers nothing to slow
it down, so it arrives at the creek as a spike rather than a swell.

### What a stormwater system is actually for

This surprises people, and it is the single most useful misconception to
correct.

A storm drain is not a sewer. It does not go to a treatment plant. The
system that collects street and roof runoff is called a municipal separate
storm sewer system, and EPA defines it as a conveyance or system of
conveyances owned by a public entity, designed or used to collect or convey
stormwater, that is "not a combined sewer" and "not part of a sewage
treatment plant."

Separate is the operative word. And EPA is direct about the consequence:
"Polluted stormwater runoff is commonly transported through municipal
separate storm sewer systems (MS4s), and then often discharged, untreated,
into local water bodies."

So the purpose of a storm drain is to move water away from where it would
cause a problem, quickly. That is a flooding solution. It is not a water
quality solution, and in most places it is the opposite of one, because it
takes runoff that would have spread out and soaked in across a mile of
ground and delivers it to one pipe outlet in one concentrated slug.

Anything you put on a hard surface goes in that pipe. It is worth saying to
yourself once, in those words, the next time you wash a car on a driveway.

## The largest remaining water quality problem is ordinary people

This is the sentence in this guide that most deserves to be read twice.

EPA on nonpoint source pollution: "NPS pollution is caused by rainfall or
snowmelt moving over and through the ground. As the runoff moves, it picks
up and carries away natural and human-made pollutants, finally depositing
them into lakes, rivers, wetlands, coastal waters and ground waters."

And then the verdict: "States report that nonpoint source pollution is the
leading remaining cause of water quality problems."

Leading. Remaining. Both words are doing work.

A point source is a pipe. You can find it, permit it, meter it, fine it,
and in the United States that has largely been done. What is left is the
pollution that has no pipe, because it arrives off everything at once,
every time it rains.

Here is EPA's own list of what nonpoint source pollution consists of:

- "Excess fertilizers, herbicides and insecticides from agricultural lands
  and residential areas"
- "Oil, grease and toxic chemicals from urban runoff and energy production"
- "Sediment from improperly managed construction sites, crop and forest
  lands, and eroding streambanks"
- "Salt from irrigation practices and acid drainage from abandoned mines"
- "Bacteria and nutrients from livestock, pet wastes and faulty septic
  systems"
- "Atmospheric deposition and hydromodification"

Read that list looking for the villain and you will not find one. There is
no factory on it. There is a lawn, a driveway, a dog, a construction site,
a pasture and a septic tank. The fourth item mentions residential areas in
the same breath as agricultural ones.

EPA's urban runoff list is even closer to home. Sediment. Oil, grease and
toxic chemicals from motor vehicles. Pesticides and nutrients from lawns
and gardens. Viruses, bacteria and nutrients from pet waste and failing
septic systems. Road salts. Heavy metals from roof shingles and motor
vehicles. And thermal pollution from impervious surfaces such as streets
and rooftops, which is the one nobody thinks of: a July rain shower that
crosses hot asphalt arrives at the creek warm, and warm water holds less
oxygen than cold.

The consequence, in EPA's words, is that "these pollutants can harm fish
and wildlife populations, kill native vegetation, foul drinking water, and
make recreational areas unsafe and unpleasant."

None of that requires anybody to do anything wrong. It is the aggregate of
a lot of people doing nothing in particular, on ground that no longer soaks
anything up.

## Septic systems, and the counterintuitive thing about them

Roughly speaking, a septic system is a small sewage treatment plant that
uses soil as the treatment stage. Understanding it in exactly those terms
explains everything else about it.

EPA describes the sequence. "All water runs out of your house from one main
drainage pipe into a septic tank." The tank is "a buried, water-tight
container usually made of concrete, fiberglass, or polyethylene," whose job
is "to hold the wastewater long enough to allow solids to settle down to
the bottom forming sludge, while the oil and grease floats to the top as
scum." The liquid in the middle goes on to the drainfield, which is "a
shallow, covered, excavation made in unsaturated soil."

Then the sentence that carries the whole design: "The soil accepts, treats,
and disperses wastewater as it percolates through the soil, ultimately
discharging to groundwater."

The tank is a settling chamber. The soil is the treatment plant. Which
means the system does not work because you bought it. It works because the
ground under it is the right kind of ground, and if it is not, the system
is a pipe that puts sewage into an aquifer.

### Why the soil decides

Unsaturated is the key word in that drainfield definition. Effluent has to
move down through air-filled pores slowly enough for soil organisms,
adsorption and filtration to work on it. Two failure modes follow directly,
and they are opposites.

**Too slow, and it backs up.** EPA: "If the drainfield is overloaded with
too much liquid, it can flood, causing sewage to flow to the ground surface
or create backups in toilets and sinks." This is what happens over dense
till, over clay, and anywhere a seasonal water table rises into the trench.
EPA names the site conditions that cause it: systems installed in areas
with "inadequate or inappropriate soils, excessive slopes, or high ground
water tables."

**Too fast, and it is not treated.** The effluent leaves before the soil
has done anything to it, and arrives in the groundwater carrying what it
started with. EPA's blunt version: "Insufficiently treated sewage from
septic systems can cause groundwater contamination."

That second one is the counterintuitive case, and Silverdale has it in
textbook form.

Look at the two big soils here and their septic ratings. Alderwood, on half
the land, rates Very limited for a drainfield because of wetness at 120 to
180 cm plus slope on the steeper phases. That is the too slow failure, and
it is the one everybody expects.

Indianola, on 10.3 percent of the land, also rates Very limited. But the
survey's reason is entirely different: "seepage through the bottom layer
and inadequate filtering in the 60-150 cm filter field zone," and the
locale data adds the interpretation in one line, which is worth quoting
because it is the whole point of this section: "This is a
groundwater-contamination limitation, not a drainage failure."

Indianola is hydrologic group A. Deep sand, no restrictive layer, very high
conductivity. It is the best-draining ground in the area. And that is
exactly why it fails the septic test. The effluent is not going to back up.
It is going to leave, fast, barely treated, into the aquifer that the
neighbourhood drinks.

A **Very limited** septic rating on a well-drained soil is a groundwater
contamination finding. On a poorly drained soil it is a drainage failure.
Same two words, opposite problems, opposite fixes.
[Your Soil, Specifically](your_soil_specifically.md) covers how to read
those ratings and, more importantly, the reason printed next to each one.
The reason is the part that tells you which of these two you have.

The survey's answer for the whole area is unusual and worth knowing: septic
drainfields rate Very limited on essentially every major series here, for
opposite reasons depending on which block you are standing on. Too slow on
the till and lake-sediment soils, too fast and unfiltered on the outwash
sands. There is no default good ground for a drainfield in Central Kitsap.

EPA notes that other designs exist for exactly these situations: "Mound
systems are an option in areas of shallow soil depth, high groundwater, or
shallow bedrock," and system design varies with "household size, soil type,
site slope, lot size, proximity to sensitive water bodies, weather
conditions, or even local regulations." Very limited does not mean
forbidden. It means engineered, and expensive.

### What failure looks like

EPA's list of symptoms, and every one of them is visible without
instruments:

- "Water and sewage from toilets, drains, and sinks backing up into the
  home's plumbing"
- "Bathtubs, showers, and sinks draining very slowly"
- "Standing water or damp spots near or over the septic tank or drainfield"
- "Sewage odors around the septic tank or drainfield"
- "Bright green, spongy lush grass over the septic tank or drainfield, even
  during dry weather"

That last one is the sly one. It does not look like a failure at all. It
looks like the healthiest grass on the property, and in August, when
everything else here is brown, it is the only green patch in the yard.

The consequences reach past your own house. EPA lists "High levels of
nitrates or coliform bacteria in surface waters or drinking water wells"
and "Algae blooms in nearby lakes or waterbodies" among the signs that
somebody's system has failed, and notes that "malfunctioning septic systems
release bacteria, viruses, and chemicals toxic to local waterways" and that
improperly treated sewage "poses the risk of contaminating nearby surface
waters, and potentially cause various infectious diseases in swimmers, from
eye and ear infections to acute gastrointestinal illness and hepatitis."

The maintenance that prevents most of this is unglamorous and cheap. EPA:
"In general, a septic tank should be inspected every 1 to 3 years and
pumped every 3 to 5 years," and "regular septic system maintenance fees of
$250 to $500 every three to five years is a bargain compared to the cost of
repairing or replacing a malfunctioning system, which can cost between
$5,000 and $15,000." The reason systems fail, in EPA's summary, is
"inappropriate design or poor maintenance," and neglect lets "solids in the
tank to migrate into the drain field and clog the system."

A tank you have never pumped is not a tank that has been fine. It is a tank
whose sludge layer has been climbing for twenty years toward the outlet
pipe, at which point the drainfield, the expensive half, starts filling
with solids it was never meant to receive.

## Where the down path goes: recharge

Water that infiltrates does not vanish. USGS: "Some water that infiltrates
will remain in the shallow soil layer, where it will gradually move
vertically and horizontally through the soil and subsurface material. Some
of the water may infiltrate deeper, recharging groundwater aquifers." The
saturated zone beneath the water table is the aquifer, and aquifers "are
huge storehouses of water."

That storehouse is not a separate system from the creeks. Groundwater "is
discharged from the aquifer from springs, seeps into streams, or is
withdrawn by wells." The water in the ground and the water in the channel
are the same water at different points in its trip.

On the Kitsap Peninsula this stops being an abstraction, because there is
nothing else. USGS describes the hydrologic setting as similar to that of
an island. The peninsula is surrounded by saltwater, there is no upstream
river to divert from, and groundwater is the normal domestic water source
and the only one. **Every litre of fresh water on this peninsula fell on
this peninsula.**

The budget makes that concrete. For 2012, an above average precipitation
year, USGS put recharge from precipitation across the Kitsap Peninsula at
664,610 acre-feet, about 820 million cubic metres, plus another 22,122
acre-feet returning from water that had been used. Where it goes:

| Fate of recharge | Share |
|---|---|
| Discharges to streams | 66 percent |
| Discharges to Hood Canal and Puget Sound | 30 percent |
| Withdrawn from wells | 4 percent |

Four percent is pumped. Two thirds of it is what keeps the creeks running.

That second number is the one to sit with, because it means the well and
the creek are drawing on the same account. USGS's household version of the
same fact: heavy pumping "can even cause your neighbor's well to run dry if
you both are pumping from the same aquifer." At watershed scale the
neighbour is the creek.

And the creeks here have no slack left in summer. Chico Creek, the only
stream in the Dyes Inlet basin with a long flow record, averages 92.24
cubic feet per second in January and 1.99 in August. That is 2.612 cubic
metres per second against 0.056. The largest local stream runs at about one
forty-sixth of its winter flow at the end of summer, and the smaller ones,
Koch and Strawberry, can be assumed to do worse. Extra withdrawal in
August comes out of a flow that is already nearly nothing.

Washington has legislated on exactly this point. The statewide groundwater
permit exemption allows a domestic withdrawal of up to 5,000 gallons per
day without a water right. But Silverdale sits in WRIA 15, one of eight
watersheds named in state law for streamflow restoration, and there a new
domestic permit-exempt withdrawal is capped at an annual average of 950
gallons per day per connection, about 3,596 litres. That is 19 percent of
the statewide allowance. Under a drought emergency order the cap drops to
350 gallons per day, indoor use only, though water for a fire control
buffer is still allowed.

There is a detail in that law that belongs in this guide specifically: an
applicant "must manage stormwater runoff on-site to the extent practicable
by maximising infiltration." The state has written the thesis of this
document into the conditions on a well permit. If you are going to take
water out of the ground here, you are required to try to put rain back into
it.

Those are interim statutory measures that apply until rules say otherwise,
and Ecology adopted the WRIA 15 watershed plan in December 2024 with
rulemaking to follow, so confirm the current number before you rely on it.

## Seawater intrusion, and why it does not undo

On a coast, over-pumping does not just lower a water table. It changes what
comes out of the pipe.

Fresh groundwater and saltwater meet underground at what USGS calls the
freshwater/saltwater interface. "Under natural conditions, the seaward
movement of freshwater prevents saltwater from encroaching on freshwater
coastal aquifers." The fresh water flowing out to sea is what holds the
salt back. It is a pressure balance, not a wall.

Take enough fresh water out and the balance moves. USGS: "Groundwater
pumping can reduce freshwater flow toward coastal areas and cause saltwater
to be drawn toward the freshwater zones of the aquifer." The same agency
lists saltwater contamination among the effects of groundwater depletion:
"pumping can cause saltwater to migrate inland and upward, resulting in
saltwater contamination of the water supply."

The outcome is not a taste complaint. USGS: "Saltwater intrusion decreases
freshwater storage in the aquifers, and, in extreme cases, can result in
the abandonment of wells." Washington's Department of Ecology says some
coastal wells in the state are now unusable because of seawater intrusion,
and names Island, Jefferson, Clallam and Pacific counties among the coastal
areas at risk. Ecology attributes it to natural processes such as sea level
rise and, principally, to over-pumping of wells near saltwater coastlines
that draw from aquifers at or below sea level.

**Why this is different from most contamination.** A tank leak can be dug
out. A failing drainfield can be replaced. But the salt is not sitting in a
spot, it is dissolved through a body of rock and sand, and the only thing
that removes it is fresh water pushing it back out, which is the same slow
flow that was there in the first place. Turning the pump off stops making
it worse. It does not make it better on any timescale a household plans
around. **Treat this one as a decision you get to make once.** That framing
is a rule of thumb, not an agency statement; the sourced facts are the
abandonment of wells and the unusable coastal wells Ecology reports.

The Silverdale-specific news is good, with an asterisk. The most detailed
federal study of the peninsula found no widespread seawater contamination
of wells, and listed the local areas where chloride in well water exceeds
25 mg/L: the southern Longbranch peninsula, Horsehead Bay, Point Evans,
Sinclair Inlet, Eagle Harbor, Fletcher Bay, the north end of Bainbridge
Island and the north tip of the Kitsap peninsula. Silverdale and Dyes Inlet
are not on that list. Sinclair Inlet, 11 km south, is.

The asterisk is the date. That finding rests on 1980 field data. It is the
most specific published federal statement available and it is 45 years old,
so a shoreline well should be tested for chloride rather than assumed
clean. Local well depths give some idea of what you would be testing: near
Silverdale the median recorded depth is 98 feet, about 30 metres, with half
of local wells between 41 and 255 feet, and seasonal water level swings in
the unconsolidated units run 1 to 20 feet.

## The shellfish are the receipt

Everything above is a mechanism. This is the consequence, and it is the
sharpest example of downstream anybody has, because somebody publishes a
number for it.

The locale water file lists thirteen surface waters around Silverdale.
Twelve of them are fresh: Clear Creek and its west fork, Strawberry Creek,
Barker Creek, Koch Creek, Chico Creek, Dickerson Creek, Wildcat Creek,
Mosher Creek, and the lakes Island, Kitsap and Wildcat. Every one of the
twelve drains, directly or through another, into the thirteenth.

The thirteenth is Dyes Inlet, and Dyes Inlet is saltwater, which means it
is the only one of the thirteen that grows shellfish, and therefore the
only one carrying a shellfish growing area classification. Silverdale sits
at its head.

Washington's Department of Health classifies commercial growing areas into
four categories, and the definitions are worth knowing because they encode
a diagnosis, not just a verdict.

- **Approved** means the sanitary survey found no contamination presenting
  an actual or potential public health hazard. Harvest is allowed.
- **Conditionally Approved** means the area meets the Approved standard
  most of the time but predictably fails at certain times, such as after
  heavy rain. It closes for a predetermined period based on how long the
  water takes to recover.
- **Restricted** means the water does not meet the Approved standard, but
  the pollution appears to come from non-human sources. Shellfish must be
  moved to an Approved area to cleanse themselves before sale.
- **Prohibited** means the survey indicates fecal material, pathogenic
  microorganisms or poisonous or harmful substances may be present at
  concentrations that pose a health risk. No commercial harvest.

Note what Conditionally Approved really says. It is not a hedge. It is a
statement that the department knows a rainstorm will make that water unsafe
and knows roughly how long it stays that way. The classification is a
measurement of runoff, written on a map.

Here is Dyes Inlet, as of 4 September 2026.

| Classification | Acres | Share |
|---|---|---|
| Approved | 102.91 | 2.8 percent |
| Conditionally Approved | 1,291.10 | 34.5 percent |
| Restricted | 0 | 0 percent |
| Prohibited | 2,344.74 | 62.7 percent |
| **Total** | **3,738.75** | **15.13 km2** |

Just under two thirds of the inlet at the bottom of these creeks is closed
outright. Under three percent of it is open without conditions.

The reasons the department gives for the prohibited areas are "Wastewater
Treatment Plant Outfall, Nonpoint Pollution" and, for the rest, "Nonpoint
Pollution" on its own. The conditional closures trigger on combined sewer
overflow events and raw sewage discharges during specific tides.

Nonpoint pollution. The same category EPA calls the leading remaining cause
of water quality problems, the one whose ingredient list is fertilizer,
pet waste, livestock, road runoff and faulty septic systems. That is what
closed most of Dyes Inlet. Not one outfall. Everybody's yard.

The department finds this out by walking the shoreline. A sanitary survey
combines a shoreline survey that identifies pollution sources including
sewage plants, septic systems, animal farms, drainage and wildlife, with
year-round marine water sampling for fecal coliform bacteria, plus analysis
of how weather and tides move the contamination around. Somebody is
literally walking the creek mouths counting the sources.

So the causal chain is short and complete, and it runs both ways:

**A failing drainfield, or livestock with creek access, or a dog run by a
ditch, puts bacteria in runoff. Runoff reaches a creek. The creek reaches
the inlet. Shellfish filter the water and concentrate what is in it. The
beach closes. It closes for everybody, including the neighbours who did
nothing, including you.**

And the reverse: the classification map is a public readout of what the
land around it is doing. If you want to know whether the watershed above a
creek is clean, you do not need to test the creek. Somebody already tested
the water the creek runs into, and published it.

Two things this map does not tell you, which the
[Silverdale gazetteer](../locale/silverdale_wa.md) covers in full and which
must be checked separately on the day. First, marine biotoxins are a
different hazard with a different authority and no season at all, and
cooking and freezing do not destroy them; check the Department of Health
shellfish safety map or the hotline on 1-800-562-5632 every time. Second,
recreational beach seasons are set separately by the state fish and
wildlife department. Growing area classification, biotoxin status and open
season are three independent gates, and none of them implies another.

## What one household can actually do

Honestly ranked, best first. The ordering is a rule of thumb assembled from
the sourced mechanisms above, not a published hierarchy from any agency,
but the logic behind it is not controversial: every step down this list is
more work and less effective than the one above it.

**1. Keep it out of the water in the first place.** Nothing else on this
list is as cheap or as effective, because runoff is a transport system and
the only guaranteed fix is to have nothing for it to transport. Concretely:
pick up dog waste, which is on EPA's urban runoff list by name. Keep
livestock out of creeks and off the ground next to them. Use less
fertilizer and none before rain, since excess fertilizer from residential
areas is the first item on EPA's nonpoint source list. Do not wash cars,
tip paint, or empty anything onto a driveway or into a storm drain, which
goes to the creek untreated. And pump the septic tank, every three to five
years, which is the single highest-value maintenance item on the property
for reasons that end at a beach.

**2. Slow it down.** A given volume of water does far less harm arriving
over six hours than over twenty minutes, because slow water erodes less,
carries less, and has time to soak in. Anything that puts roughness,
vegetation or a longer path between the downspout and the ditch does this.

**3. Spread it out.** Concentrated flow cuts channels and finds the fastest
route. Dispersed flow uses the whole soil surface. Splitting one downspout
into several outlets across a lawn does more than it looks like it should.

**4. Let it soak in.** This is the goal, and it is last because it is the
one the ground has a veto over. EPA's Soak Up the Rain programme names the
standard measures: disconnect downspouts, rain gardens, rain barrels,
permeable pavement, green roofs, and trees. All of them work. How well they
work on your lot is set by the hydrologic soil group under it, which is why
that table is earlier in this guide than this list is.

On Alderwood, a rain garden is fighting a hardpan at 56 cm, and it will
pond in February when the perched water table is already up. On Indianola,
the same rain garden drains beautifully and you should think about what
else is going down with the water, since that soil is rated Very limited
for a drainfield precisely because things pass through it unfiltered. Both
of those are still better than a pipe to the street. Neither is a
catch-all.

The honest summary for a wet-winter, dry-summer place like this one: the
water you want to infiltrate arrives in the months when the ground is
already full, and the months when the ground would happily take it are the
months with no rain. That is not a reason to do nothing. It is a reason to
expect the greatest benefit from the first two items on this list rather
than the fourth, and to pair infiltration with storage, which
[Collecting Rainwater](collecting_rainwater.md) covers.

## Doing all of this where you actually live

The Silverdale numbers are a worked example. The method is the point, and
every step of it exists for the whole country.

**1. Find your watershed.** Start with EPA's How's My Waterway at
`https://mywaterway.epa.gov/`, which reports the condition of local waters
for a place you name, including your local watershed, drinking water
information, and identified issues. Then get the boundary and code itself
from the USGS Watershed Boundary Dataset. Write down your 12-digit HUC. It
is the most compact way to say where you are, hydrologically, and it is the
key that unlocks most other datasets.

**2. Trace it outward.** Find what your unit drains into, and what that
drains into, until you reach the sea. Silverdale takes five steps. Some
places take fifteen. The exercise takes ten minutes and it permanently
changes how a storm drain looks.

**3. Get your soil's hydrologic group.** The soil survey has it, and
[Your Soil, Specifically](your_soil_specifically.md) is the guide to
getting it out. A, B, C or D, plus the depth to any restrictive layer,
tells you what your ground does with rain before you spend a cent on
drainage.

**4. Look up your impairments.** Under the Clean Water Act, "states are
charged with assessing the condition of their waters and compiling the
303(d) list of those waters impaired by pollutants every two years." That
list is public, and it names the cause of each impairment. If your creek is
on it for fecal coliform, you have just learned what the neighbourhood's
septic systems are doing.

**5. Find the downstream receipt.** Every coastal state health department
classifies shellfish growing areas, and inland the equivalents are beach
closure notices, fish consumption advisories and drinking water source
assessments. Whatever the local version is, it is the public record of what
the land upstream is delivering, and it is usually free and usually
updated.

**6. Find out what happens to your own runoff.** Walk the property during
actual rain, which is the only time any of this is visible. Where does the
downspout discharge? Where does the water go from there? Does it reach a
ditch, a drain, a creek, or does it soak in? That walk answers more than
any map, and almost nobody takes it.

## What can go wrong

- **You assumed the storm drain goes to a treatment plant.** It is a
  separate system and its contents are commonly discharged untreated.
- **You judged your drainage by texture instead of depth.** Sandy soil with
  a hardpan at 40 cm is in the highest runoff class, exactly like clay.
- **Your septic rating said Very limited and you assumed that meant
  soggy.** On fast-draining soil it means the opposite: the effluent leaves
  before it is treated, into the aquifer you drink from.
- **You have never pumped the tank because nothing has gone wrong.**
  Nothing going wrong is what the drainfield clogging quietly looks like,
  right up until the expensive half of the system is dead.
- **You dug a rain garden without checking the soil first.** On a perched
  water table in winter it is a pond, and in a fast outwash sand it is an
  unfiltered injection point for whatever is in the runoff.
- **You solved your own flooding by moving water faster.** Faster water is
  somebody else's problem two hundred metres downhill, and it is the
  mechanism that closed the beach.
- **You are on a coastal well and you drilled deeper when the water got
  low.** On a peninsula, deeper and harder pumping near the shoreline is
  how the salt arrives, and that one does not undo.
- **You assumed the beach closure was about the treatment plant.** Most of
  it, at Dyes Inlet and generally, is nonpoint pollution, which is a polite
  way of saying it is everybody.

## How you know it worked

- You can name your watershed and give its 12-digit HUC.
- You can trace, step by step, where water leaving your property ends up,
  and name the last body of water before the ocean.
- You know the hydrologic soil group under your house and the depth to any
  restrictive layer, and therefore roughly what your ground does with an
  inch of rain.
- You know where every downspout on your building discharges, because you
  have watched them in the rain.
- If you are on septic, you know when the tank was last pumped, what the
  survey rates your soil for a drainfield, and which of the two failure
  modes that rating refers to.
- If you are on a coastal well, you have a chloride result, not an
  assumption.
- You can name at least one specific thing downstream of you that would be
  affected if your land shed dirty water, and you can say who checks it.

When those are true, you have stopped thinking of your property as the
place the water arrives and started thinking of it as one segment of a
route. That is the transferable part. The particular creek changes
everywhere you go; the fact that you are standing in the middle of
somebody's water supply does not.

## Sources

Grouped by what kind of authority each one is. United States government
publications are listed first because they are public domain and can be
redistributed with this guide; the rest cannot, so every fact taken from
them is restated here in our own words.

### United States government (public domain)

- United States Geological Survey, Water Science School. Watersheds and
  Drainage Basins (the definition of a watershed, the outflow point, nested
  watersheds, and the drainage divide).
  https://www.usgs.gov/special-topics/water-science-school/science/watersheds-and-drainage-basins
- United States Geological Survey. Watershed Boundary Dataset (what the WBD
  is, hydrologic units, the progressive two-digit HUC scheme, the eight
  levels and 2- to 16-digit codes, completeness to 12 digits, and
  boundaries drawn without regard to jurisdiction).
  https://www.usgs.gov/national-hydrography/watershed-boundary-dataset
- United States Geological Survey, Water Science School. Infiltration and
  the Water Cycle (precipitation as the greatest controlling factor,
  saturated soil, clay versus sand, vegetation, slope, impervious surfaces
  as a fast lane, and infiltration recharging aquifers).
  https://www.usgs.gov/special-topics/water-science-school/science/infiltration-and-water-cycle
- United States Geological Survey, Water Science School. Surface Runoff and
  the Water Cycle (the one-third runoff and two-thirds evaporated,
  transpired or infiltrated split; the meteorological and physical factors;
  urbanisation replacing vegetated land with impervious surfaces).
  https://www.usgs.gov/special-topics/water-science-school/science/surface-runoff-and-water-cycle
- United States Geological Survey, Water Science School. Evapotranspiration
  and the Water Cycle (definitions of evaporation and transpiration; an
  acre of corn at 3,000 to 4,000 gallons a day; a large oak at 40,000
  gallons a year).
  https://www.usgs.gov/special-topics/water-science-school/science/evapotranspiration-and-water-cycle
- United States Geological Survey, Water Science School. Aquifers and
  Groundwater (the aquifer as the saturated zone below the water table,
  confined and unconfined, recharge from precipitation, discharge to
  springs and streams, and one well drawing down a neighbour's).
  https://www.usgs.gov/special-topics/water-science-school/science/aquifers-and-groundwater
- United States Geological Survey, Water Science School. Groundwater
  Decline and Depletion (long-term water-level declines from sustained
  pumping; reduction of water in streams and lakes; deterioration of water
  quality; saltwater migrating inland and upward).
  https://www.usgs.gov/special-topics/water-science-school/science/groundwater-decline-and-depletion
- United States Geological Survey. Saltwater Intrusion (the
  freshwater/saltwater interface; seaward freshwater movement preventing
  encroachment; pumping drawing saltwater toward freshwater zones;
  decreased storage and, in extreme cases, abandonment of wells).
  https://www.usgs.gov/mission-areas/water-resources/science/saltwater-intrusion
- USDA Natural Resources Conservation Service. National Engineering
  Handbook, Part 630 Hydrology, Chapter 7, Hydrologic Soil Groups (the
  descriptions and conductivity limits for groups A, B, C and D; the least
  transmissive layer principle; the 50 cm impermeable layer and 60 cm water
  table criteria; the conductivity range of an impermeable layer; dual
  groups).
  https://directives.nrcs.usda.gov/sites/default/files2/1712930597/11905.pdf
- United States Environmental Protection Agency. Basic Information about
  Nonpoint Source Pollution (the definition, the rainfall and snowmelt
  mechanism, the full list of nonpoint source pollutants and their sources,
  and nonpoint source as the leading remaining cause of water quality
  problems).
  https://www.epa.gov/nps/basic-information-about-nonpoint-source-nps-pollution
- United States Environmental Protection Agency. Nonpoint Source: Urban
  Areas (impervious surfaces increasing the volume and velocity of runoff;
  the urban pollutant list including pet waste, failing septic systems,
  roof shingle metals and thermal pollution; the harms).
  https://www.epa.gov/nps/nonpoint-source-urban-areas
- United States Environmental Protection Agency. Stormwater Discharges from
  Municipal Sources (the definition of an MS4, that it is not a combined
  sewer and not part of a treatment works, and that polluted stormwater is
  often discharged untreated into local water bodies).
  https://www.epa.gov/npdes/stormwater-discharges-municipal-sources
- United States Environmental Protection Agency. How Your Septic System
  Works (the main drainage pipe, the tank and its sludge and scum layers,
  the drainfield as an excavation in unsaturated soil, the soil accepting,
  treating and dispersing wastewater to groundwater, and drainfield
  flooding). https://www.epa.gov/septic/how-your-septic-system-works
- United States Environmental Protection Agency. Types of Septic Systems
  (the conventional tank and drainfield; mound systems for shallow soil,
  high groundwater or shallow bedrock; the factors that drive system
  design). https://www.epa.gov/septic/types-septic-systems
- United States Environmental Protection Agency. What to Do if Your Septic
  System Fails (the five failure symptoms; inappropriate design or poor
  maintenance; inadequate soils, excessive slopes and high ground water
  tables; nitrates and coliform in surface waters and wells; algae blooms).
  https://www.epa.gov/septic/what-do-if-your-septic-system-fails
- United States Environmental Protection Agency. Why Maintain Your Septic
  System (inspection every 1 to 3 years and pumping every 3 to 5; the $250
  to $500 against $5,000 to $15,000 comparison; insufficiently treated
  sewage causing groundwater contamination; the illnesses).
  https://www.epa.gov/septic/why-maintain-your-septic-system
- United States Environmental Protection Agency. Soak Up the Rain (the
  named household measures: disconnecting downspouts, green roofs,
  permeable pavement, rain barrels, rain gardens and trees).
  https://www.epa.gov/soakuptherain
- United States Environmental Protection Agency. How's My Waterway (what
  the tool is for and the community-level information it returns, including
  local watershed water quality, drinking water information and identified
  issues). https://www.epa.gov/waterdata/hows-my-waterway and the tool
  itself at https://mywaterway.epa.gov/
- United States Environmental Protection Agency. Impaired Waters and TMDLs,
  Frequent Questions (states compiling the Clean Water Act section 303(d)
  list of waters impaired by pollutants every two years; what a TMDL is).
  https://www.epa.gov/tmdl/impaired-waters-and-tmdls-frequent-questions

### Washington State agencies (cited as the authority, restated in our own words)

Washington is a state, not the federal government, so these publications
are ordinary copyright. Their facts are used here; their wording is not.

- Washington State Department of Health. Shellfish Growing Areas (the
  Approved, Conditionally Approved, Restricted and Prohibited
  classifications and what each means; the sanitary survey combining
  shoreline survey, year-round marine water sampling for fecal coliform,
  and analysis of weather and tides; the pollution sources surveyed).
  https://doh.wa.gov/community-and-environment/shellfish/growing-areas
- Washington State Department of Health. Commercial Shellfish Growing Areas
  feature service, layer "growingareas" (the Dyes Inlet acreages and
  classification shares, as of 4 September 2026, held in the locale water
  file).
  https://services8.arcgis.com/rGGrs6HCnw87OFOT/arcgis/rest/services/Commercial_Shellfish_Growing_Areas/FeatureServer/0
- Washington State Department of Ecology. Seawater Intrusion (what it is;
  sea level rise and over-pumping of wells near saltwater coastlines
  drawing from aquifers at or below sea level; coastal wells in Washington
  now unusable; the counties at risk).
  https://ecology.wa.gov/water-shorelines/water-supply/water-rights/seawater-intrusion
- Washington State Department of Ecology. Groundwater Permit Exemption (the
  5,000 gallons per day statewide domestic exemption).
  https://ecology.wa.gov/water-shorelines/water-supply/water-rights/groundwater-permit-exemption
- Revised Code of Washington 90.94.030 (WRIA 15 named among the eight
  streamflow restoration watersheds; the 950 gallons per day annual average
  cap per connection, the 350 gallons per day drought curtailment with the
  fire control buffer allowance, and the requirement to manage stormwater
  on-site by maximising infiltration).
  https://app.leg.wa.gov/RCW/default.aspx?cite=90.94.030

### Inside this project

- `data/locales/silverdale_wa/water.json`, which holds every Silverdale
  water figure quoted here with its provenance: the WRIA and HUC chain and
  the drainage path to the ocean from the USGS Watershed Boundary Dataset;
  the thirteen surface waters and their drainage areas and gage records
  from USGS GNIS and NWIS, including the Chico Creek monthly means for 1947
  to 1974; the Dyes Inlet shellfish classification from Washington DOH; the
  Kitsap Peninsula aquifer framework, well depth distribution, seasonal
  water level fluctuation and 2012 water budget from USGS Scientific
  Investigations Report 2014-5106 (Welch, Frans and Olsen, 2014,
  https://pubs.usgs.gov/sir/2014/5106/); the seawater intrusion findings
  from USGS Open-File Report 80-1186 (Hansen and Bolke, 1980,
  https://pubs.usgs.gov/publication/ofr801186); and the precipitation
  normals from NOAA NCEI.
- `data/locales/silverdale_wa/soil.json`, for the hydrologic soil groups,
  septic ratings and restrictive layer depths of the Silverdale series,
  drawn from the USDA NRCS SSURGO database for survey areas WA635 and
  WA778.
- [Where Water Comes From](where_water_comes_from.md), the companion guide
  on sources and their risks, and the origin of the habit of looking
  upstream.
- [Your Soil, Specifically](your_soil_specifically.md), for how to look up
  a soil survey, what a restrictive layer is, and how to read a Very
  limited rating and the reason printed next to it.
- [Where You Are: Silverdale, Washington](../locale/silverdale_wa.md), the
  generated gazetteer, for the rainfall figures used in the roof
  arithmetic, the wet-half-of-the-year pattern, and the biotoxin and
  shellfish season gates that sit alongside the growing area
  classification.
- [Collecting Rainwater](collecting_rainwater.md), for storing what you
  catch rather than only slowing it down.

### Labelled in the text as rules of thumb, not sourced

- The ranking of household actions, best first: keep it out of the water,
  slow it down, spread it out, let it soak in. The mechanisms behind each
  step are sourced above; the ordering is ours.
- The framing of seawater intrusion as a decision you get to make once. The
  sourced facts are that pumping draws saltwater inland, that it can end in
  the abandonment of wells, and that some Washington coastal wells are now
  unusable. How long an intruded aquifer takes to flush is not stated by
  any source opened for this guide, so no figure is given for it.
- The observation that tidal basins flush more slowly than rivers, so what
  enters them stays in the neighbourhood longer. The drainage chain itself
  is from the Watershed Boundary Dataset; the inference about residence
  time is ours.

### Arithmetic rather than citation

The roof and driveway volumes are geometry, not measurements. One
millimetre of rain on one square metre of surface is one litre of water, so
every figure in that section is a rainfall depth multiplied by an area:
150 m2 by 1,446 mm gives 216,900 litres, by 252 mm gives 37,800, by 21 mm
gives 3,150, and by 25 mm gives 3,750. The rainfall depths are sourced; the
multiplication is not a claim about anything.
