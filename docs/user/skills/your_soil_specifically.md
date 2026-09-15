# Your Soil, Specifically

Somebody has already dug a hole in your garden. They described what they
found, gave it a name, wrote down how deep the roots go and how long the
water sits, and published the result. In most of the United States this
was done decades ago, it cost you nothing, and you can read it this
afternoon by typing your address into a government website.

Almost nobody does. People buy amendments, lose fruit trees, dig
foundations, and argue about drainage for years without ever looking up
the answer that has been sitting there the whole time.

This guide is about that lookup. How to do it, what the answer means,
where it stops being true, and what you still have to find out for
yourself. It does not teach what soil is made of. If you cannot yet
judge texture by hand or say what a horizon is, read
[What Soil Is](what_soil_is.md) first: it covers sand, silt and clay,
structure, compaction, horizons, pH and the ribbon test, and this guide
assumes all of it.

The worked example here is Silverdale, Washington, because there is a
real sourced soil file behind it. The method is the point and the method
works anywhere the survey has been made.

## The answer you want is a name

Ask most people what their soil is and they will say "clay" or "sandy"
or, hopefully, "loam". That is a texture, and a texture describes one
property of one layer. It is a useful thing to know and it is not the
answer.

The answer is a **soil series** name. Alderwood. Indianola. Norma.
Kapowsin. A series is the whole profile treated as one described object:
the layers in order, their depths, what they are made of, how deep you
can dig before something stops you, how fast water leaves, how acid it
is, and what grew there before anyone farmed it.

That matters because the useful facts about ground are not properties of
a handful of dirt. They are properties of the profile. Two soils can
both be sandy loam at the surface and be completely different
propositions, because one of them has a metre of more sandy loam
underneath and the other has a slab of concrete-hard glacial till at
half a metre. You cannot feel that difference in your palm. The series
name carries it.

So a series name is a prediction. Given the name, you can say in advance:

- how fast this ground drains, and whether it holds water in winter
- how deep roots and excavators can go before they hit something
- how much water the soil can store for a plant between rains
- roughly how acid it is
- what the surveyors rated it as suitable and unsuitable for

None of which you get from the word "loam".

## Doing it: Web Soil Survey

The tool is Web Soil Survey, run by the USDA Natural Resources
Conservation Service at `https://websoilsurvey.nrcs.usda.gov/`. NRCS
describes it as access to the largest natural resource information
system in the world, with coverage for more than 95 percent of the
nation's counties.

It is also a website designed around 2007 and it shows. It opens in a
second window, it drives everything through four numbered tabs, it sells
you a free product through a shopping cart, and it refuses to show you
anything at all until you have drawn a box on a map. None of that is
your fault, and none of it means you are doing it wrong. Here is the
path through it.

**1. Press Start WSS.** The tool opens in its own window with four tabs
across the top: Area of Interest, Soil Map, Soil Data Explorer, Shopping
Cart. NRCS describes them as, in order, where you define your area of
interest, where you view or print a soil map and detailed descriptions
of the soils, where you access soil data and determine suitability, and
where you get your custom printable report.

**2. Define an Area of Interest, and understand that this is a separate
step.** This is where people get stuck. Finding your house on the map is
not the same as setting an AOI. Navigate first, using the Quick
Navigation panel on the left (address is the fastest route; you can also
pick a state and county, or a whole soil survey area) or just by zooming.
Then you must actively draw the area with one of two toolbar tools, one
for a rectangle and one for an irregular polygon. Nothing else in the
tool works until an AOI exists.

Draw it a little larger than your property. You want to see what your
parcel sits inside, not just what is under the house.

**3. Soil Map tab.** Orange lines appear over your area: these are map
unit boundaries. The legend on the left lists each map unit by symbol
and name, with the acres inside your AOI and the percentage of the AOI
it covers. The name is based on the predominant soil series in that unit.

This is the moment you have been after. You now know what your ground is
called.

**4. Click the map unit name.** You get the map unit description: which
soil components make it up and in what percentage, the slope range, a
typical profile with horizon depths, the drainage class, depth to the
water table, depth to any restrictive layer, and the farmland
classification. Read this one carefully. It is the densest page in the
whole system.

**5. Soil Data Explorer tab, for the interpretations.** Two subtabs earn
their keep. **Suitabilities and Limitations for Use** gives the ratings
for dwellings with and without basements, local roads and streets,
septic tank absorption fields, shallow excavations and small commercial
buildings. **Soil Properties and Qualities** gives available water
capacity, depth to a restrictive layer, pH and the rest as numbers.

**6. Shopping Cart tab.** Everything you added along the way comes out
as a Custom Soil Resource Report, a PDF, free. Generate it and keep it.
Most of it is boilerplate; the map unit descriptions and the two or three
interpretations you actually care about are the part to read.

Set aside half an hour the first time. The second time it takes five
minutes.

## Then open the Official Series Description

Web Soil Survey tells you what is on your ground. The **Official Series
Description**, or OSD, tells you what that soil *is*, nationally, in the
words of the people who defined it. They live at
`https://soilseries.sc.egov.usda.gov/` and you can search by name.

An OSD gives you the taxonomic class, a typical pedon with every horizon
and its depth and colour, the range in characteristics across the whole
extent of the series, drainage and hydraulic conductivity, use and
natural vegetation, where the series occurs and how extensive it is, and
a competing series section naming the soils it is nearly identical to
and what separates them.

The Alderwood OSD, to make this concrete, gives the taxonomic class as
"Loamy-skeletal, isotic, mesic Aquic Dystroxerepts", a typical pedon
running from a gravelly sandy loam A horizon at 0 to 18 cm down through
very gravelly sandy loam to a dense glacial till at 89 cm, a depth to
densic contact of 50 to 100 cm, a reaction of 5.1 to 6.5, and moderately
well drained status with high conductivity above the densic contact and
low conductivity in the densic material itself.

Read that once and you know the shape of half the land in Kitsap County.

Two honest warnings. OSDs are written by soil scientists for soil
scientists, and they are not consistent with each other: Alderwood's is
in centimetres and Indianola's is in inches. And they express things
differently from the tabular data. Alderwood's OSD gives reaction as
the numbers 5.1 to 6.5; Indianola's gives it as "neutral to strongly
acid", a class range with no numbers at all, while the survey's own
tables give Indianola pH 5.1 to 6.8.

So use the OSD for the shape of the profile, the depth of the thing that
stops roots, and the vegetation it formed under. Take precise numeric
ranges from the tables in Web Soil Survey. And read the competing series
section: learning what distinguishes your soil from its near neighbours
is the fastest way to understand what actually matters about it.

## A map unit is not your property

Everything above is an average of a polygon. Your quarter acre is a
point inside it. Those are different things, and the survey says so
itself in plain language.

**The unit is named for its dominant component, not its only one.** The
Soil Survey Manual defines a consociation as a map unit dominated by a
single soil component, where most of the remainder of the delineation
consists of soil so similar to the named soil that major interpretations
are not affected significantly. "Most of the remainder" is not "all of
it", and "not affected significantly" is a judgement somebody made.

**Some units tell you outright that they are mixtures.** A complex or an
association contains two or more dissimilar components occurring in a
regularly repeating pattern, interleaved too finely to separate at the
mapping scale. When you see a name like Urban land-Alderwood complex, or
Alderwood-Kitsap complex, or Indianola-Kitsap complex, the name is
telling you that two different soils are braided together under your
feet and the map cannot say which one you are standing on.

**Minor components add up.** In the Silverdale data, the Everett series
is never a major component of any map unit in the area. It appears only
as a 5 to 10 percent inclusion inside Alderwood and Indianola units. It
still accounts for about 3 percent of the land. A map unit called
Alderwood is not all Alderwood.

**And the survey prints a warning on the map.** Every Custom Soil
Resource Report carries this text: "Warning: Soil Map may not be valid
at this scale. Enlargement of maps beyond the scale of mapping can cause
misunderstanding of the detail of mapping and accuracy of soil line
placement. The maps do not show the small areas of contrasting soils
that could have been shown at a more detailed scale."

The same report says it again in the narrative: "If intensive use of
small areas is planned, however, onsite investigation is needed to
define and locate the soils and miscellaneous areas."

Your garden bed is an intensive use of a small area.

### Check the map against the ground with a spade

The map is a hypothesis. Here is how to test it, and it takes an
afternoon.

Dig at least three holes, well apart, in the parts of the property you
actually intend to use. Not one hole. The whole point is to find out
whether the ground is uniform, and one hole cannot answer that.

Dig each one as deep as you can, and at minimum past the depth the
description gives for its restrictive layer. Then compare what you see
against what you read:

- Do the horizon depths and colours roughly match the typical profile?
- Where does the gravel start, and does it match?
- Where do the roots stop, and do they turn sideways when they get
  there?
- Does the spade hit something it cannot get through, and at what depth?

A restrictive layer announces itself. The spade stops, the material is
dense and often grey, and roots mat out sideways along the top of it
rather than crossing it.

**Rule of thumb, not a published method:** a steel probe rod or a
digging bar is far faster than a spade for mapping a hardpan across a
yard. Push it in at a rough grid of points, record the depth at which it
stops, and you have the shape of the pan under your whole garden in an
hour. A spade tells you what the layers look like; the bar tells you
where they are.

**Rule of thumb on timing:** if you want to know about a seasonal water
table, dig in the wet season. A perched water table is an event, not a
permanent feature, and in August it is simply not there to find. In
Silverdale that means digging between about December and May.

If the ground disagrees with the map, the ground wins. Every time. The
map is a very good prior and it is not a measurement of your yard.

## The four things worth reading carefully

A map unit description contains dozens of fields. Four of them carry
most of the weight for a person who wants to grow something or build
something.

### Drainage class

Drainage class is the soil's **natural** drainage: the condition it
formed in, before anyone dug a ditch or ran a downspout. There are seven
classes, defined in the Soil Survey Manual. The three that appear most
around Silverdale read like this.

*Somewhat excessively drained* means water is removed from the soil
rapidly, and internal free water is commonly very rare or very deep.
What you experience: it never floods, it warms early in spring, and it
is dry by July.

*Moderately well drained* means water is removed somewhat slowly during
some periods of the year, with internal free water commonly moderately
deep. What you experience: fine for most of the year, soggy in the wet
season, and you cannot work it in winter without ruining the structure.

*Poorly drained* means water is removed so slowly that the soil is wet
at shallow depths periodically during the growing season or remains wet
for long periods, with internal free water shallow or very shallow and
common or persistent. What you experience: standing water, dead roots on
anything that is not adapted to it, and a wetland permit conversation if
you try to change it.

The class is a measure of how long air is excluded from the root zone.
That is the thing that actually kills plants, as
[What Soil Is](what_soil_is.md) explains: water and air share the same
pore space, and a soil full of water is a soil with no air in it.

### Depth to a restrictive layer

This is the single most predictive number for anyone digging anything.

A restrictive layer is anything roots and water cannot readily get
through. It might be bedrock, but far more often it is not. Around Puget
Sound it is **densic material**: dense glacial till, described in the
Keys to Soil Taxonomy as relatively unaltered, compact, root-restrictive
material with a noncemented rupture resistance class, commonly formed of
dense glacial till or volcanic mudflow. Locally, everyone calls it
hardpan.

Note the word noncemented. Alderwood's till is dense enough to stop
roots and water but it is not cemented, so a machine can dig it, with
effort. That is different from a genuinely cemented layer, and the
survey distinguishes them: Kapowsin carries a weakly cemented horizon at
about 64 cm sitting above its densic material at about 74 cm, which is
two restrictions rather than one.

What the depth number predicts, concretely:

- the deepest a root can go, which caps drought tolerance for anything
  perennial
- where winter water will perch and sit
- what an excavator hits, and therefore what a foundation or a trench
  costs
- whether a septic drainfield has room to work below the pipe

A tree does not care what the survey calls the layer. It cares that at
56 cm there is nothing left to grow into.

### Available water capacity

Available water capacity is the water a soil can hold that a plant can
actually take up: the water held between field capacity, what remains
after free drainage, and the permanent wilting point, below which the
plant cannot pull any more out. Web Soil Survey reports it as a depth of
water stored within a given depth of soil, which sounds abstract and is
the most practically useful number in the whole report.

It answers: how long can this ground go without rain before the plants
in it are in trouble?

Two things reduce it that people do not expect. Gravel and stones hold
no water at all, so a very gravelly soil has far less capacity than its
texture suggests. And a restrictive layer truncates it, because water
stored below a layer roots cannot cross is water the plant will never
get.

The Silverdale numbers make the point better than any explanation.
Kitsap silt loam stores 28.6 cm of available water in the top 150 cm.
Alderwood, on half the land, stores 8.7 cm. Shelton stores 5.2. Those
three soils sit within a few miles of each other and one of them holds
more than five times as much water as another.

### pH range

The survey gives a pH range for each layer, and it is worth reading for
what it is: the reaction of the soil as it formed, under its natural
vegetation, at the time it was described.

It is a good prediction of your starting point and it is not your
current number. Lime, fertilizer, irrigation water, and forty years of
somebody else's lawn all move pH. Testing is the next section.

For what the ranges mean and why most crops want the slightly acid to
neutral band, see [What Soil Is](what_soil_is.md).

### A fifth thing: what the ratings vocabulary means

The suitability ratings use three words and they are more precise than
they look. **Not limited** means the soil is favourable for that use.
**Somewhat limited** means the limitation can be overcome with design
changes or extra cost. **Very limited** means, in the survey's own
words, that the limitations generally cannot be overcome without major
soil reclamation, special design, or expensive installation procedures,
and that poor performance and high maintenance can be expected.

Very limited does not mean forbidden. It means expensive. And the rating
always comes with a reason, which is the part to read: "ponding",
"depth to saturated zone", "slope", "seepage, bottom layer". The reason
tells you what you would have to fix, and whether fixing it is possible.

## Silverdale, worked

Here is what all of that looks like on one real place.

The headline is a single fact of geometry. About 64 percent of the
mapped land around Silverdale is a thin gravelly mantle lying on dense
glacial till. Alderwood alone accounts for 49.9 percent of the mapped
land, and Kapowsin, Harstine, McKenna, Shelton and the Kapowsin variant
account for the rest of the till soils. The hardpan is not a local
quirk. It is the character of this place, and it explains nearly every
practical problem here.

The survey areas involved are WA635, Kitsap County Area, which covers
essentially all of the town, and a small sliver of WA778, Bangor Naval
Station, in the northwest. Shares below are percentages of mapped land:
Dyes Inlet is water and is not soil-mapped at all.

### Alderwood: half the ground, and a contradiction

Alderwood is a gravelly sandy loam at the surface over very gravelly
sandy loam, sitting on dense noncoherent glacial till. The restriction
is at a representative 56 cm, with a mapped range of 50 to 102 cm. It is
moderately well drained, with a seasonal perched water table as shallow
as 65 cm. pH 5.1 to 6.5. And it stores only 8.7 cm of plant-available
water in the top 150 cm.

Read the perched water table and the 8.7 cm together and you get the
contradiction that defines gardening here: **winter-wet and
summer-droughty on the same ground.**

The mechanism is the pan. Silverdale gets about 1446 mm of rain a year
and 79.8 percent of it falls between October and March. That water
reaches the till, which is nearly impermeable, and stops. So it perches,
and the root zone is saturated for months. Then July brings 21 mm of
rain, the thin gravelly mantle above the pan is all the storage there
is, and 8.7 cm does not last. The same soil that drowned your roots in
February starves them in August.

What it predicts:

- **Growing food.** Plan to irrigate through July and August. Plan to
  lime. Build upward rather than trying to dig downward, because in the
  typical case there is nothing useful below about 56 cm. Deep-rooted
  perennials and fruit trees are the hard case: they will hit the pan.
  Farmland class ranges by phase from prime-if-irrigated to not prime.
- **Digging a foundation.** Dwellings with basements rate Very limited,
  for perched ground water at 75 to 180 cm plus slope on the steeper
  phases. Shallow excavations rate Very limited for wetness and the
  dense layer at 50 to 150 cm. Dwellings without basements, local roads
  and small commercial buildings rate Somewhat limited on the flatter
  phases.
- **A drainfield.** Very limited, for slope plus wetness at 120 to 180
  cm. The effluent has nowhere to go.

### Indianola: the mirror image

Indianola, 10.3 percent of the land, is loamy sand to sand the whole way
down. Very deep, no restriction at all, somewhat excessively drained,
very high saturated hydraulic conductivity. pH 5.1 to 6.8, 9.7 cm of
available water.

Its drainfield rating is Very limited too, and this is the detail worth
understanding, because the reason is the exact opposite of Alderwood's.
The survey's objection is seepage through the bottom layer and
inadequate filtering in the 60 to 150 cm filter field zone. The effluent
is not going to back up. It is going to leave, fast, barely treated.

That is a groundwater contamination limitation, and on the Kitsap
Peninsula it is not academic: groundwater is the normal domestic water
source, and the only one, because as USGS puts it the hydrologic setting
is similar to that of an island. The thing you are protecting is the
water you drink. See [Where Water Comes From](where_water_comes_from.md)
for what that means on the drinking end.

Shallow excavations also rate Very limited here, for unstable excavation
walls. Sand caves in. That is a safety rating, not a convenience one.

For growing: droughty, low natural fertility, and lime and nutrients
leach away quickly. It warms early in spring and suits root crops, and
it is prime farmland only if irrigated. Unirrigated cropping in July and
August is not realistic.

### Norma: the trap

Norma is 6.1 percent of the land, and it is the one that catches people
out, because it looks like the best ground on the property. Dark, deep,
7.5 percent organic matter in the surface, 21 cm of available water,
classed prime farmland.

Prime farmland **only if drained**. It is poorly drained with an
apparent water table at 0 cm, meaning at the surface, and it ponds in
winter and spring. It is a hydric soil.

Everything rates Very limited: dwellings with and without basements for
ponding over 4 hours and ground water at the surface, local roads for
ponding and wetness, septic for wetness and ponding and restricted
percolation, small commercial buildings and shallow excavations for the
same. If you find Norma under the flattest, richest-looking corner of
your land, that corner is a pond for part of the year.

And hydric means wetland rules apply, so draining it is a permitting
question before it is an engineering one.

The wet organic soils here carry an even sharper warning. Shalcar is
muck holding 50.3 cm of available water, and Mukilteo is peat holding
67.5 cm. Both are rated prime farmland if drained. Both are also rated
for more than 30 to 60 cm of total **subsidence**, which is the survey
telling you that if you drain them, the peat oxidises and the ground
surface drops by half a metre. Do not build on them and do not drain
them casually.

### Kapowsin: the one actually worth farming

Kapowsin is 5.3 percent of the land and the only series in this area
classed "All areas are prime farmland". Gravelly ashy loam over loam,
with 6 percent organic matter in the plough layer and 14.2 cm of
available water. Ashy means volcanic ash influenced, which is why it is
friable and pleasant to work. pH 5.1 to 6.5.

Its catch is the same family of problem as everything else here, just
deeper down: a weakly cemented horizon at about 64 cm and densic
material at about 74 cm. Deep-rooted perennials hit it and so do
drainfields, which rate Very limited for wetness at 120 to 180 cm.

If you are choosing land around here to grow food on, this is the soil
to look for.

### Kitsap: why the phase matters as much as the series

Kitsap, 4.2 percent, is the highest water-holding mineral soil in the
area at 28.6 cm, a silt loam over silty clay loam formed in glacial lake
sediments. Its flat 2 to 8 percent phase is classed prime farmland.

The same series is also mapped on terrace escarpments around Dyes Inlet
at slopes up to 70 percent, including an Indianola-Kitsap complex at 45
to 70 percent, where slope rates as a limitation for every single use.

Same name. Two completely different propositions. Which is why the
**phase**, the slope and erosion class appended to the map unit name,
deserves as much attention as the series itself. "Kitsap silt loam, 2 to
8 percent slopes" and "Kitsap silt loam, 30 to 70 percent slopes" are
not the same answer.

### The short version

| Series | Share | Restriction | Available water in 150 cm | pH | The one thing to know |
|---|---|---|---|---|---|
| Alderwood | 49.9% | Dense till, 56 cm typical (50 to 102 cm) | 8.7 cm | 5.1 to 6.5 | Winter-wet and summer-droughty on the same ground |
| Indianola | 10.3% | None, very deep sand | 9.7 cm | 5.1 to 6.8 | Drains too fast to filter a drainfield |
| Norma | 6.1% | None, but the table is at the surface | 21 cm | 5.6 to 7.3 | Looks like the best ground, is a hydric wetland |
| Kapowsin | 5.3% | Cemented at 64 cm, densic at 74 cm | 14.2 cm | 5.1 to 6.5 | The only all-prime farmland here |
| Kitsap | 4.2% | None, but slow percolation and a winter table | 28.6 cm | 5.6 to 7.3 | Prime when flat, cliff-grade when not |
| Urban land | 0.7% | Not rated | not rated | unknown | Not a soil. Go and look |

### What it adds up to

**For a grower.** Lime is not optional on most of this ground: the
common soils run pH 5.1 to 6.5, and Cathcart and Tacoma reach 4.5.
Summer irrigation is required almost everywhere. The two soils genuinely
worth farming are Kapowsin and the flat phases of Kitsap.

**For a builder.** The binding constraint in Central Kitsap is not
bedrock. Only one series in the whole area, Schneider at 0.3 percent,
has a lithic contact at all. It is dense till at roughly two to three
feet plus a perched winter water table, and that combination puts
basements at Very limited across almost the entire area.

**For a drainfield.** Very limited on essentially every major series
here, for opposite reasons depending on which block you are on: too slow
on the till and lake-sediment soils, too fast and too unfiltered on the
outwash sands.

## Urban land, fill and made ground

Around the Silverdale commercial core the survey maps an Urban
land-Alderwood complex, about 0.7 percent of the area.

Urban land is not a soil. It is a miscellaneous area: pavement,
buildings, engineered fill. The survey carries no interpretations for
it, which is the survey being honest rather than being unhelpful. What
"Urban land" actually means is **nobody knows, go and look.**

That is worth taking seriously well beyond the units actually named
Urban land, because most built lots have been rearranged. NRCS itself
notes that the distribution of soils and important soil properties in
urban areas is more variable than in undeveloped and native landscapes,
that the level of detail available for urban soils varies, and that only
a handful of cities have comprehensive urban soil survey information.
Human-transported material, the survey's term for soil somebody moved,
is associated with building sites, mining and dredging operations,
landfills and similar activities.

So on any developed parcel, assume the top layer is not what the survey
describes. It has been scraped, graded, filled, driven over by machinery
and possibly imported from somewhere else entirely. The survey describes
the soil that formed there. It cannot describe what a bulldozer left.

Fill announces itself when you dig:

- an abrupt, knife-sharp boundary between layers rather than a gradual
  one
- mixed or mottled colours with no consistent pattern down the profile
- fragments of brick, concrete, asphalt, glass or plastic
- a hard compacted layer at exactly the depth machinery would have
  tracked
- horizons out of order, such as subsoil sitting on top of topsoil

None of that is in the database. All of it is findable in an hour with a
spade.

## What the survey cannot tell you

Four things, and they are the four most likely to change.

**Your pH today.** The survey gives a range for the soil under its
natural vegetation at the time it was described. Lime, fertilizer,
irrigation water and decades of management all move it. If you intend to
correct pH, you need your own current number, not a range from a survey.

**Nutrients.** The survey carries texture, water, drainage and depth. It
does not carry your nitrogen, phosphorus, potassium or micronutrient
levels. Those are not properties of the soil type, they are properties
of what has been done to your particular ground.

**Compaction.** The bulk density figures in the database describe the
soil as it was described. The pan your builder's excavator made, or the
one under the path you walk on every day, is not in there. Find it with
a spade or a probe rod, as above.

**Contamination.** Not in the soil survey at all, in any form. It is a
different question answered by a different authority.

### Where to get it tested

For pH and nutrients, a laboratory test is cheap and the results are
specific to the sample you sent. Start with your state's cooperative
extension service and ask them where to send a sample.

One honest local note, because this varies by state and the common
advice is wrong here. In Washington, **WSU does not provide soil
testing**. It maintains lists of laboratories and states plainly that
inclusion in the list does not imply endorsement. Some other states'
land grant universities do run their own lab. Either way, extension is
the right first phone call, because they know which labs serve your area
and which report format your local advice is written against.

### And lead, which is a different question

Contamination matters most near old houses and roads, and the one to
check first is lead.

EPA reports that higher levels of lead are typically found in soil near
roadways, from decades of leaded gasoline exhaust; around the exterior
of older buildings, particularly along the dripline, that used
lead-based paint or had lead gutters and flashing; and near hazardous
waste sites, lead smelters, battery processing sites and industrial
areas. Their advice is to test, and that the best method is sending
samples to a laboratory qualified to determine lead concentrations.

For growing food, EPA recommends keeping produce at least 10 feet away
from buildings, roads and driveways, and using raised beds or containers
of clean soil where levels are higher. They also note that root
vegetables such as carrots and leafy greens take up more lead than
fruiting crops such as tomatoes, peppers and cucumbers.

For a number to compare a result against: in January 2024 EPA lowered
its recommended screening level for lead in soil at residential
properties from 400 parts per million to 200 ppm, and said it would
generally use 100 ppm at residential properties with multiple sources of
lead exposure. Be precise about what that is. It is the level at which
EPA decides to investigate a residential property further under the
Superfund and hazardous waste cleanup programmes, not a garden safety
standard. But it is the clearest federal statement of what counts as a
concerning concentration, and it is the number your lab result is
usefully read against.

[What Soil Is](what_soil_is.md) covers the rest of the contamination
basics.

## How you know it worked

- You can say the name of the soil under your own house, and the name of
  the one under your garden if they differ.
- You know the depth at which something stops a root on your land, and
  you found that depth with a spade or a probe rod, not only on a map.
- You can say how much water your soil stores and therefore roughly how
  long it can go between waterings.
- You have read the ratings for the uses you actually care about, and
  you know the reason given for each limitation, not just the word.
- You have a current laboratory pH and nutrient result for the ground
  you intend to grow food in, and if the ground is near an old house or
  a road, a lead result too.

When those five are true you are no longer guessing about your ground,
and you are not trusting a map further than the map trusts itself. The
survey gave you a very good hypothesis for free. The spade and the
laboratory told you where it was wrong.

## Sources

Grouped by what kind of authority each one is. United States government
publications are listed first because they are public domain and can be
redistributed with this guide; the rest cannot, so every fact taken from
them is restated here in our own words.

### United States government (public domain)

- USDA Natural Resources Conservation Service. Web Soil Survey (the tool
  itself; the four tabs and what each does; coverage of more than 95
  percent of the nation's counties; the description of it as the largest
  natural resource information system in the world).
  https://websoilsurvey.nrcs.usda.gov/app/HomePage.htm and
  https://websoilsurvey.nrcs.usda.gov/app/MainHelp.htm
- USDA Natural Resources Conservation Service. Custom Soil Resource
  Report, standard text carried by every report the tool generates (the
  "Warning: Soil Map may not be valid at this scale" box, and the
  statement that onsite investigation is needed if intensive use of
  small areas is planned).
  https://websoilsurvey.nrcs.usda.gov/app/
- USDA Natural Resources Conservation Service. Soil Survey Manual,
  Agriculture Handbook 18, Chapter 3, Examination and Description of
  Soil Profiles (the seven natural soil drainage classes and their
  definitions).
  https://www.nrcs.usda.gov/sites/default/files/2022-09/SSM-ch3.pdf
- USDA Natural Resources Conservation Service. Soil Survey Manual,
  Chapter 4, Soil Mapping Concepts (map units; the definition of a
  consociation; complexes and associations as two or more dissimilar
  components in a regularly repeating pattern).
  https://www.nrcs.usda.gov/sites/default/files/2022-09/SSM-ch4.pdf
- USDA Natural Resources Conservation Service. Soil Survey Manual,
  Chapter 8, Interpretations (the Not limited, Somewhat limited and Very
  limited rating classes, and the definition of Very limited as
  requiring major soil reclamation, special design or expensive
  installation procedures).
  https://www.nrcs.usda.gov/sites/default/files/2022-09/SSM-ch8.pdf
- USDA Natural Resources Conservation Service. Official Series
  Description, ALDERWOOD series (taxonomic class, typical pedon, depth
  to densic contact 50 to 100 cm, reaction 5.1 to 6.5, drainage and
  conductivity above and within the densic material).
  https://soilseries.sc.egov.usda.gov/OSD_Docs/A/ALDERWOOD.html
- USDA Natural Resources Conservation Service. Official Series
  Description, INDIANOLA series (Dystric Xeropsamments; horizons given
  in inches; reaction stated as neutral to strongly acid; somewhat
  excessively drained with very high saturated hydraulic conductivity).
  https://soilseries.sc.egov.usda.gov/OSD_Docs/I/INDIANOLA.html
- USDA Natural Resources Conservation Service. Official Series
  Description, KAPOWSIN series (the weakly cemented 3Bstm horizon at 64
  to 74 cm and the densic 3Cd below it; moderately well drained with
  very slow permeability at the cemented horizon).
  https://soilseries.sc.egov.usda.gov/OSD_Docs/K/KAPOWSIN.html
- USDA Natural Resources Conservation Service. Official Series
  Description index, for looking up any series by name.
  https://soilseries.sc.egov.usda.gov/
- USDA Natural Resources Conservation Service. Keys to Soil Taxonomy
  (the definition of densic materials as relatively unaltered, compact,
  root-restrictive material of noncemented rupture resistance,
  commonly dense glacial till).
  https://www.nrcs.usda.gov/resources/guides-and-instructions/keys-to-soil-taxonomy
- USDA Natural Resources Conservation Service. Urban Soils and Urban
  Soil Survey (soils in urban areas are more variable than in
  undeveloped landscapes; only a handful of cities have comprehensive
  urban soil survey information; human-transported material and where
  it comes from).
  https://www.nrcs.usda.gov/conservation-basics/natural-resource-concerns/soil/urban-soils
  and
  https://www.nrcs.usda.gov/conservation-basics/natural-resource-concerns/soil/urban-soil-survey
- USDA Natural Resources Conservation Service. Soil Quality Indicators:
  Available Water Capacity (available water capacity as the water held
  between field capacity and the permanent wilting point).
  https://www.nrcs.usda.gov/sites/default/files/2022-10/nrcs142p2_051590.pdf
- United States Environmental Protection Agency. Lead in Soil (where
  lead concentrates, testing through a qualified laboratory, the 10 foot
  setback for food crops, raised beds, and which crops take up the most
  lead). https://www.epa.gov/lead/lead-soil
- United States Environmental Protection Agency. Updated Residential
  Soil Lead Guidance for CERCLA Sites and RCRA Corrective Action
  Facilities, January 2024 (the screening level lowered from 400 ppm to
  200 ppm, and 100 ppm where there are multiple sources of lead
  exposure).
  https://www.epa.gov/system/files/documents/2024-01/olem-residential-lead-soil-guidance-2024_signed_508.pdf
  and
  https://www.epa.gov/superfund/residential-soil-lead-directive-cercla-sites-and-rcra-hazardous-waste-cleanup-facilities

### University extension (copyrighted; cited as the authority, restated here in our own words)

- University of Delaware Cooperative Extension, Using the Web Soil
  Survey (the two AOI drawing tools, rectangular and custom polygon; the
  orange map unit boundaries; and what the Soil Map legend lists, namely
  the map unit symbol, the name based on the predominant series, the
  acres in the AOI and the percentage of the AOI covered).
  https://www.udel.edu/academics/colleges/canr/cooperative-extension/fact-sheets/web-soil-survey/
- Washington State University Puyallup, Soils and Laboratory Testing
  (WSU does not itself provide soil testing, and maintains lists of
  laboratories with the note that inclusion does not imply endorsement).
  https://puyallup.wsu.edu/soils/archive-wsu-puyallups-legacy-of-urban-and-organic-systems-research/soils/

### Inside this project

- `data/locales/silverdale_wa/soil.json`, which holds every Silverdale
  soil figure quoted here, together with its provenance. Its underlying
  source is the USDA NRCS SSURGO database for survey areas WA635, Kitsap
  County Area, and WA778, Bangor Naval Station, queried through Soil
  Data Access and area-weighted across the Silverdale bounding box.
- [Where You Are: Silverdale, Washington](../locale/silverdale_wa.md),
  the generated gazetteer, for the soil table in summary form and for
  the rainfall figures used above: about 1446 mm a year, 79.8 percent of
  it between October and March, and 21 mm in July.
- [What Soil Is](what_soil_is.md) for texture, structure, compaction,
  horizons, the pH classes and the ribbon test, none of which are
  repeated here.
- [The Growing Calendar Where You Live](the_growing_calendar.md) for the
  frost dates and soil temperature thresholds that decide when the
  ground described here can actually be planted.
- [Where Water Comes From](where_water_comes_from.md) for what it means
  that groundwater is the only domestic water source on this peninsula.

### Labelled in the text as rules of thumb, not sourced

- Using a steel probe rod or digging bar on a grid to map the depth of a
  hardpan across a yard. The technique is ordinary practice and nobody
  has published it as a method.
- Digging test holes in the wet season if you want to observe a seasonal
  perched water table. The seasonality itself is in the survey data; the
  advice to time your digging around it is ours.
- The list of signs that a layer is fill rather than natural soil
  (abrupt boundaries, mixed colours, construction fragments, horizons
  out of order). Each sign is standard field practice; the list as given
  is not quoted from any single publication.
