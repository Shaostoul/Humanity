# How an Ecosystem Holds Together

Stand on the Dyes Inlet shoreline at low tide in November and look
around without naming anything.

There is a heron out on the flat, motionless. There is a black band of
mussels on the pilings. There is a raft of dark ducks sitting over the
middle of the inlet doing nothing in particular. Behind you the ground
rises into second-growth Douglas-fir, and where the creek comes out
there is a ragged line of pale trunks that are not conifers. Up that
creek, fish that went to sea as fingerlings are lying dead on the
gravel.

None of that is arranged for scenery. Each of those things is where it
is because of what it eats, what eats it, or what it needs to stand in.
The heron is on the flat because small fish are on the flat. The ducks
are over the middle because there is a shellfish bed under them. The
pale trunks at the creek mouth are alder because alder is what arrives
first on wet disturbed ground, and the fir behind you is fir because
that slope was cleared long enough ago for the alder to have already
come and gone.

This guide is about reading that. Not memorising a food chain, but
understanding the few rules that decide what can live where, why pulling
out one piece moves others, and why your particular place works the way
it does.

The worked examples come from `data/locales/silverdale_wa/species.json`,
which ships with this project: 122 plants, fungi, mammals, birds, fish
and shellfish of one real place on Puget Sound. Where a claim rests on a
record in that file, the record id is given so you can go and read it.
Everything else is sourced at the bottom.

A companion note before starting. This is not a foraging guide and it
does not qualify you to eat anything. For how to identify a living thing
checkably and find out what protects it, read
[The Species Where You Live](the_species_where_you_live.md) first.

## Everything begins with where the light lands

The single most useful question about any place is: where does the
energy come in?

In the forest behind you, it comes in at the top. The trees hold their
leaves in the canopy, and everything below the canopy is living on
whatever light is left over, or on material that fell from above. That
one fact drives the whole structure of the forest. It is why the Forest
Service ranks tree species by shade tolerance at all: which tree can
regenerate under a closed canopy is the question that decides what the
forest turns into. Western hemlock is near the top of that ranking, with
only Pacific yew and Pacific silver fir considered equal or more
tolerant, and the Forest Service notes that if several centuries pass
without a major disturbance, a climax of self-perpetuating, essentially
pure western hemlock can result. Red alder sits at the other end: it is
shade intolerant, so alder stands are self-thinning, and any tree that
does not hold its place in the canopy dies.

Out on the water in front of you, energy comes in at the surface, and
the thing capturing it is not a plant you can see. NOAA Fisheries
describes phytoplankton as responsible for nearly all primary production
in the Northwest continental shelf ecosystem, and states the consequence
plainly: how many organisms can live and grow in a given area depends on
the amount of primary production the phytoplankton generate.
Phytoplankton blooms are a major component of the food web and
a primary food source for zooplankton and for filter feeders such as
shellfish. These organisms range in size from under one micrometre to
over one hundred.

Hold those two pictures side by side, because almost everything else
follows from the difference.

**The forest's producers are enormous and slow.** A Douglas-fir is a
single organism holding decades of captured carbon upright in one place.
The structure itself is habitat: a trunk is a nest site, a snag is a
cavity, a fallen log is a seedbed. Change something and the forest
answers over decades. A hemlock climax is measured in centuries.

**The inlet's producers are microscopic and fast.** There is almost no
standing structure in the water column. Whatever structure exists in the
marine system has to be built by something that is not the producer: a
mussel bed, an eelgrass meadow, a gravel beach. And because the
producers are so small and so short-lived, the marine system answers
within a season. A change in light, nutrients or water movement shows up
that year.

That is why the two halves of this place fail differently. You can
degrade a forest for years before it looks wrong. You can degrade an
inlet over one wet season and see it.

## Nutrients go round, and the step people forget is rot

Energy flows through a system once and leaves as heat. Nutrients do not.
Nitrogen, phosphorus, calcium and the rest go round and round, and a
place is fertile in proportion to how well that loop closes.

Most people can name the first half of the loop: roots take up
nutrients, plants build tissue, animals eat plants, animals eat animals.
Almost nobody spends any time on the half that matters most, which is
what happens when all of those things die.

Decomposition is not the end of the story. It is the step that makes the
rest of the story possible. Until something breaks a dead tree back down
into its parts, the nutrients in that tree are locked up and unavailable
to everything else.

The National Park Service describes the mechanism for the forest you are
standing in. Saprophytic fungi break down trees as their mycelium
burrows inside to consume the tree's cellulose and lignin. That is a
fungus eating wood, and the reason it matters is that very little else
can. Wood is mostly cellulose and lignin, and lignin in particular is
difficult to take apart. A forest without its wood-rotting fungi would
slowly bury itself in undigested trunks.

You can see both halves of this in the shipped data. The oyster mushroom
(`oyster_mushroom`) grows in shelving clusters on dead and dying
hardwood, which around here means alder and maple logs along the creeks.
That is a decomposer, working. The king bolete (`king_bolete`) and the
chanterelle (`pacific_golden_chanterelle`) do something else entirely:
they grow from the soil attached to living tree roots. The Park Service
describes that trade directly. Trees share carbon with the fungi, and
the fungi supply phosphorus and nitrogen, the nutrients trees need to
thrive.

Read that again, because it upends the usual picture. The big tree is
not simply drawing nutrients out of the soil with its roots. It is
paying a fungus, in sugar, to go and get them. A great deal of what you
would call "the tree feeding itself" is actually a trade.

There is one more practical consequence. Look at the chanterelle record:
it does not grow on wood, on stumps or in bark mulch, and that fact is
part of how you rule out its poisonous lookalike. The ecology is the
identification. Knowing that one fungus eats dead wood and the other
lives on living roots tells you where each can possibly be, which is a
safety fact as much as an ecological one.

The soil half of the loop, the physics of pore space and the arithmetic
of organic matter, is set out in
[What Soil Is](what_soil_is.md), with its own sources. The household
version of the same loop, running in a heap in your own yard, is
[Your First Compost](your_first_compost.md). The point to carry here is
that a nutrient loop is only as good as its slowest step, and in a wet
conifer forest that step is rot.

## The salmon, which is what makes this place strange

Now the part that makes the Pacific Northwest genuinely unusual, and the
thing to build your whole mental model of this place around.

Every nutrient loop described so far is local. Material falls, rots, and
is taken up again more or less where it fell. Salmon break that rule.
They hatch in fresh water, do nearly all their growing at sea, and then
swim back up into small creeks, spawn, and die there. They are a
conveyor belt running the wrong way: from the fertile North Pacific into
comparatively nutrient-poor freshwater and terrestrial systems, uphill,
under their own power, on a schedule.

The National Park Service gives the term for what they deliver: marine
derived nutrients, meaning nutrients acquired by an anadromous fish and
deposited in a freshwater or terrestrial ecosystem when that fish dies.
Scientists track them with the stable isotope nitrogen-15, which is more
common in marine environments than in fresh water, so an elevated
nitrogen-15 signature in something growing beside a creek is a
fingerprint of the sea.

### How much, actually

This is where you should be careful, because the numbers vary a great
deal by system and by what is being measured, and the biggest numbers
come from somewhere else.

The headline study is Helfield and Naiman, published in *Ecology* in
2001. They worked on two watersheds on Chichagof Island in southeast
Alaska, comparing sites on reaches where salmon spawn against reference
sites on the same watersheds above waterfalls or above the upstream
limit of spawning. Their result: trees and shrubs near spawning streams
derive roughly 22 to 24 percent of their foliar nitrogen from spawning
salmon. Broken out by species, their mixing model gave about 24 percent
in Sitka spruce, 22 percent in devil's club and 22 percent in fern.

The consequence they measured is the striking part, and it is about one
species, not all four. Among the Sitka spruce they cored within 25
metres of the stream, mean annual basal area growth was more than
tripled at the spawning sites. Put in a form you can picture: those
spruce would need about 86 years to reach 50 centimetres diameter at
breast height, against 307 years at the reference sites. Same species,
same island, same rainfall. Salmon.

Two honest qualifications, both of which the authors themselves raise.

First, they flag that the timescale of the enrichment is unknown. The
percentages might reflect the share of the soil nitrogen pool that
arrives from salmon each year, or they might reflect small inputs
accumulating over many years, in which case any single year's delivery
matters less than the number suggests.

Second, and this is the detail worth remembering, one plant in their
study showed no salmon signal at all. Red alder came out at about 1
percent. Their explanation is that alder gets most of its nitrogen by
fixing it out of the air, so it is less likely to take up the salmon's.
Hold on to that, because alder is about to reappear.

Now the nearest local number, and it is worth being careful about what
it is, because it is not the Washington version of the figure above.
There is no Puget Sound equivalent of the Helfield foliar measurement.
What exists measures a different compartment entirely. The Encyclopedia
of Puget Sound, published by the Puget Sound Institute at the University
of Washington Tacoma, reports work by Bilby and colleagues in 1996
finding that salmon-derived nitrogen made up 10 to 20 percent of the
nitrogen in some species of fish and invertebrates in a western
Washington salmon stream, and notes that higher proportions have been
documented in Alaskan systems.

Do not read 10 to 20 percent as a smaller version of 22 to 24 percent.
One is nitrogen in the leaves of riparian trees and shrubs in southeast
Alaska; the other is nitrogen in the bodies of fish and invertebrates in
a western Washington stream. Different organisms, different tissue,
different place. Neither number is the other one shrunk. Same mechanism,
measured at two different points in the same loop.

### How the fish gets out of the water

A salmon carcass in a creek only fertilises the creek. Something has to
carry it into the forest.

Reimchen and Fox, working on a salmon stream where black bears were the
dominant vector, recorded the scale of that transport. In a single year,
eight bears moved about 3,100 salmon, roughly 10,700 kilograms, into the
riparian zone, leaving around 330 grams of carcass remnants per square
metre. Carcasses were concentrated within 50 metres of the spawning
reaches, but the effect reached further than the carcasses did: the
growth response of trees to salmon abundance extended at least 90 metres
into the forest, and the authors think likely further. Note which
measurement that is. The nitrogen signature itself behaved the other
way, strongest beside the spawning gravel and falling off with distance
into the forest. It is the growth that showed up at 90 metres.

Flooding does the same job by a different route. Ben-David, Hanley and
Schell sampled five plant species along 18 transects running from stream
to upland forest, out to 1,000 metres, and found the marine nitrogen
signature declining with distance and elevation above the stream in
three of the five, and higher in plants at sites actively used by
fish-eating predators. Two delivery mechanisms, flood and predator, both
real.

At the far end of that range, the Park Service reports salmon nutrients
found up to seven miles from the stream of origin.

Both of those transport studies were done further north than Puget
Sound, so read them as the mechanism rather than as a measurement of
Kitsap County. Nobody has published the equivalent numbers for Chico or
Clear Creek, and there is no honest way to scale one to the other.

Around Dyes Inlet you can watch the mechanism operate in November. The
chum record (`chum_salmon`) is blunt about it: spawned-out carcasses
carry ocean nitrogen into the streamside forest, and this is a real
nutrient subsidy rather than a metaphor. The bald eagle record
(`bald_eagle`) notes that numbers rise in autumn and winter when birds
concentrate on salmon runs and on the carcasses afterward. River otters
(`river_otter`), raccoons (`raccoon`), coyotes (`coyote`) and the
occasional black bear (`black_bear`) all work the same reach. Each of
them eats a fish at the water's edge and puts part of it down somewhere
in the trees.

### And the loop closes

Helfield and Naiman finish their paper with the part that makes this a
loop rather than a one-way subsidy. Riparian forests shade streams,
filter sediment and nutrients on the way in, and supply the large woody
debris that retains gravel and shapes the channel. So salmon fertilise
the trees, and the trees build and maintain the spawning and rearing
habitat for the next generation of salmon. The authors describe it as a
positive feedback mechanism maintaining the long-term productivity of
river corridors along the Pacific coast.

Which means that a creek can fail for a reason that has nothing to do
with fishing. Cut the streamside trees and you lose the shade, the
filtration and the wood. Lose those and you lose the gravel and the cold
water. Lose those and you lose the fish, and the trees that remain lose
their nitrogen subsidy as well.

The shipped records name the specific local dependencies. Coho
(`coho_salmon`) juveniles spend about a year and a half in fresh water
before going to sea, which is why the record calls them the best single
indicator of whether a Silverdale creek is actually working: a creek
that goes warm, low or blocked in August will not hold them. Chum
(`chum_salmon`) fry move down to the estuary shortly after they emerge
and rear there for several months, which means Dyes Inlet itself is the
nursery, so filling and armouring the shoreline hits chum directly.
Beaver ponds
(`american_beaver`) on Clear Creek and Chico Creek are significant
rearing habitat for juvenile salmon, which is why dam removal is
regulated.

And at the far end of the same chain: the Southern Resident killer
whales (`killer_whale`) feed primarily on Chinook, and NOAA notes that
some populations of their preferred prey are themselves threatened or
endangered, which makes prey availability a limit on whale recovery. The
creek and the whale are the same problem.

## Keystone species, and the honest version of the story

The idea that one species can hold a whole community in place was
invented on this coast, and the invention is worth knowing accurately,
because the term has been stretched a long way since.

Robert Paine, newly hired at the University of Washington, went to
Mukkaw Bay on the outer Olympic coast in spring 1963 to lead a field
trip. He noticed a band of ochre sea stars, *Pisaster ochraceus*, below
the mussel beds, and formed a hypothesis: that local species diversity
is directly related to the efficiency with which predators prevent the
monopolisation of major environmental requisites by one species.

He tested it with a crowbar. He pried the sea stars off a stretch of
shore eight metres wide and two metres high, and kept them off. Acorn
barnacles settled heavily on the cleared rock. Then California mussels,
*Mytilus californianus*, moved in and replaced the barnacles, the algae
and the other species holding primary space. Paine wrote that the area
had become trophically simpler. The count of primary space-holding
species went from fifteen to eight.

That is the experiment. Three things about it are routinely got wrong.

**The word "keystone" is not in the 1966 paper.** Paine coined it in a
later note in the same journal, in 1969. Lafferty and Suchanek, revisiting
the work on its fiftieth anniversary, found that 22 percent of the papers
citing Paine 1966 cite it for the keystone species concept, which is not
in it.

**Most citations have lost the content.** The same review found that the
most common way to cite Paine 1966 is a generic statement that predators
increase diversity by interfering with competition. Only 10 percent
specify that the predators were sea stars and only 5 percent that the
prey were mussels. Lafferty and Suchanek put it directly: most authors
cite the paper for a brief statement, leaving out what Paine did, where
he did it, and what increased.

**And the result itself is more interesting than the slogan.** Mussels
are not merely a competitor that wins. A mussel bed is three-dimensional
habitat for over 300 associated species living inside it. So removing
the sea stars reduced the diversity of primary space occupiers, which is
what Paine counted, and increased community-wide diversity, which he did
not. Both are true. "Remove the keystone and diversity collapses" is a
tidier sentence than the data supports.

By 1993, Mills, Soule and Doak were arguing in *BioScience* that the
keystone-species concept had become a problem in its own right, under a
subtitle that is itself the argument: management and policy must
explicitly consider the complexity of interactions in natural systems.

So use the idea, but use it carefully. **Rule of thumb, not a sourced
claim:** when you hear a species called a keystone, ask what specifically
it is supposed to be holding up, whether anyone removed it and watched,
and what was counted. Nearly every real ecosystem is held together by a
dense mesh of ordinary interactions rather than one load-bearing animal,
and "keystone" is often doing the work of "this species is charismatic
and we would like it protected."

### The unplanned experiment, in your own water

In 2013 and 2014 a wasting disease swept sea stars from Mexico to
Alaska, and it hit *Pisaster ochraceus* hard in Washington. Eisenlord
and colleagues surveyed 6,568 ochre stars at 16 sites in the San Juan
Islands, South Puget Sound and the Washington outer coast between
December 2013 and July 2015. Peak disease prevalence at individual sites
ran as high as 100 percent, with an overall mean of 61 percent. Ochre
star populations declined by 67 percent on average, with 80 percent
reductions among adults. In the San Juan Islands the adult population
fell to a quarter of its pre-outbreak abundance.

The authors note that the keystone concept was founded on experimental
demonstrations in Washington that predation by ochre stars structures
rocky intertidal habitat, and that a possible consequence of the decline
is a large influx of mussels overgrowing other primary space holders,
which is exactly what the removal experiments produced.

One honest gap: the shipped Silverdale file has 14 shellfish records and
no sea star record at all. If you want to watch this in your own
intertidal, you will be working without a local entry, which is a gap in
the dataset rather than an absence of sea stars.

## What a clearing does over eighty years

Succession is the most useful ecological idea for anyone who owns
ground, because it tells you what your land will become if you do
nothing, which is the baseline every other decision is measured against.

You can watch the whole sequence around Silverdale, because most of the
peninsula is at a different point on it. Here is the local version,
built from the Forest Service reviews of the species involved and the
shipped records for where they actually grow.

**Year zero.** Something removes the canopy: logging, fire, windthrow, a
road cut, a landslide scar, a cleared building lot. Bare disturbed soil,
full sunlight.

**Year one to five.** Red alder arrives, and it arrives in absurd
numbers. The Forest Service describes disturbed areas naturally seeding
from wind-dispersed seed into stands that start with several thousand
alder trees per acre. Douglas-fir and red alder are named as the
principal pioneer tree species here. Alongside it come the things that
also want full sun and disturbed ground: salmonberry (`salmonberry`)
thickets, thimbleberry, bracken. And, on any ground near a road,
Himalayan blackberry (`himalayan_blackberry`) and Scotch broom
(`scotch_broom`), which are dealt with in their own section below.

**Year five to twenty-five.** The alder grows fast and closes a canopy,
and while it does, it is quietly manufacturing the soil the next forest
will use. Red alder fixes nitrogen from the air. The Forest Service puts
the accretion rate at 40 to 300 pounds of nitrogen per acre per year,
varying with stand location, vigour, age and density. It prints the
metric equivalent as 45 to 355 kilograms per hectare, and if you redo
that conversion yourself you will not get the same answer: one pound per
acre is 1.12 kilograms per hectare, so 300 pounds per acre is 336, not
355. The pounds-per-acre figures are the ones to carry. Anywhere a
document hands you a unit conversion, doing it yourself costs ten
seconds, and [Units and Converting Them](units_and_converting_them.md)
is about exactly this habit.

Soils under alder develop higher available and total
nitrogen. The leaves are nitrogen-rich, decompose rapidly, and form a
deep humus that improves soil structure. Conifer seedlings establish in
the shade beneath.

There is a complication worth knowing, because it is the kind of thing
that gets left out of the cheerful version. That same nitrogen
accumulation acidifies the ground. The Forest Service cites a study from
coastal Oregon in which pure alder stands averaged soil pH 4.3 to 4.4,
against 5.3 under adjacent conifer stands. Alder is not simply improving
the soil; it is changing it, and it makes it more acid while making it
richer.

That Oregon pair is a direction, not your number, and this is worth
labouring because it is the commonest way a real finding turns into bad
advice. Expect alder ground to run more acid than the conifer ground
beside it. Do not expect 4.3. The soil data for this area disagrees:
`soil.json` puts the Alderwood series, which covers about half the
mapped ground here, at pH 5.1 to 6.5, and the most acid series in the
whole file, Everett, starts at 4.5. A reader who limed to an Oregon
figure would be correcting a problem their own ground does not have.
Measure your own, and [What Soil Is](what_soil_is.md) explains what a
low pH actually does to nutrient availability.

**Year twenty-five to forty.** The conifers catch up. The Forest Service
gives it plainly: after about 25 years conifers equal red alder in
height and begin to overtop them, and after about 40 years Douglas-fir
becomes dominant. Because alder is shade intolerant, the alder stand
thins itself out as it loses the light.

**Year forty to eighty.** The alder goes. Few red alder remain in stands
past 60 years, and maximum age for the species is about one hundred.
What is left is a Douglas-fir stand standing on soil that an alder
generation built. That is the forest on nearly every upland slope around
Silverdale (`douglas_fir`): second growth, fir-dominated, with alder now
confined to the wet creek edges and the newest disturbance
(`red_alder`).

**Century scale.** If nothing disturbs it, western hemlock takes over.
It is the climax tree of these forests (`western_hemlock`), seeding
readily onto rotting logs and stumps and taking over shaded, humid
stands under and after the Douglas-fir. The Forest Service notes that
several centuries without major disturbance can produce essentially pure
western hemlock.

Two things to take from that sequence.

The first is that **the pioneer species is not a weed.** Alder looks like
scrub and it is doing the single most important piece of work in the
whole sequence. The soil series mapped across this area is even named
for it. The NRCS series description lists the potential natural vegetation
for that ground as Douglas-fir, western hemlock, western redcedar and red
alder together, which is a statement about what the site will support, not
about what the soil formed under.

The second is that **the sequence is a clock you can read backwards.** A
stand of pure alder means disturbance within about twenty-five years. Fir
over a thinning alder understorey means forty-ish. Fir with no alder left
in it means the alder has already done its job and died out. You can walk
a property line and date the disturbance without any records at all.

## Edges, and why so much happens at them

Look at where the interesting things in this place actually are, and a
pattern shows up fast. They are at boundaries.

**Forest to open ground.** The Forest Service records that red alder
dominated early seral communities in recently clearcut Douglas-fir
forest are favourable habitat for black-tailed deer, and that red
alder and thimbleberry stands in Oregon are preferred by black-tailed deer
in summer and early autumn, when daytime temperatures are highest, and are
generally avoided in winter. That second half is the useful one: the same
patch is not the same resource in February. The deer is not a deep-forest
animal. It is an edge animal, which is why you see it on the margin of a
cleared lot.

**Land to fresh water.** The belted kingfisher (`belted_kingfisher`)
needs two things at once and will not settle for one: fishable water
with a perch over it, and a bare earth bank to dig a nest tunnel two and
a half to six feet into. That is a species that lives on a line. Note
what the record points out: eroding creek banks and cut slopes, exactly
the features a tidy landowner would armour, are the nest sites.

**Land to salt water, which is the sharpest edge here.** Surf smelt
(`surf_smelt`) spawn in the upper intertidal, laying eggs in the sand and
fine gravel at the top of the beach, in the band shaded by overhanging
vegetation. WDFW flags a coastal squeeze because the backshores of those
beaches tend to be armoured with bulkheads. Pacific herring
(`pacific_herring`) spawn on nearshore vegetation, so a bulkhead that
removes the vegetation removes the spawning substrate. Pacific sand
lance (`pacific_sand_lance`) burrow into clean intertidal sand and
vanish within seconds of disturbance, so a beach can look empty and be
full of them.

All three of those are forage fish, and the herring record states the
consequence for everything else: herring feed salmon, seabirds and
marine mammals, so herring habitat sits upstream of nearly every other
animal in the dataset. A seawall is not a small local decision. It is a
decision about a band of beach that the whole inlet's food supply passes
through.

**Why edges are productive, as a rule of thumb rather than a sourced
claim.** An edge gives an animal access to two different sets of
resources without having to commit to either, and it usually has more
structure and more light than either side. That is the standard
explanation and it is a reasonable one, but treat it as a generalisation.
The specific cases above are sourced; the general rule is a frame for
looking, not a fact to quote.

And the honest other half: edges are also how things get in. The habitat
descriptions for the four worst local invaders all read the same way.
Three of them are Forest Service reviews cited below; the Himalayan
blackberry description comes from its own record, which is sourced to the
state weed board and USDA PLANTS rather than to the Forest Service. Roadsides, field margins, fence lines, forest edges,
riparian corridors, railroad grades, logged ground. Every edge you create
is both an opportunity and a door.

## A food web you can trace yourself

Here is the point of shipping a local species file rather than a regional
book. You can follow the links yourself, record to record, and check
every one.

Start at the bottom and work up.

| Link | Records that carry it |
|---|---|
| Phytoplankton feed filter feeders | `bay_mussel`, `manila_clam`, `pacific_oyster` (marine primary production per NOAA) |
| Shellfish and herring eggs feed a sea duck | `surf_scoter`: winters on mussels and clams down to 66 feet, then switches to herring eggs in spring migration |
| Forage fish feed the salmon | `pacific_herring`, `surf_smelt`, `pacific_sand_lance` feeding into `chinook_salmon`, `coho_salmon` |
| Salmon feed the top predators | `killer_whale` (primarily Chinook), `harbor_seal`, `bald_eagle`, `osprey` |
| Forage fish feed a seabird that nests inland | `marbled_murrelet`: feeds at sea on herring and sand lance, nests on mossy branches in mature conifer forest |
| Salmon carcasses feed the forest | `chum_salmon` carcasses moved ashore by `bald_eagle`, `river_otter`, `raccoon`, `coyote`, `black_bear` |
| The forest builds the creek | streamside trees supply shade, filtration and large wood back to the spawning gravel |
| Beaver ponds rear the juveniles | `american_beaver` ponds on Clear Creek and Chico Creek |
| Dead trees house the next generation | `pileated_woodpecker` excavates cavities in snags that owls, ducks and squirrels then nest in |
| Native shrubs feed the birds and the bears | `cascara`, `red_elderberry`, `bitter_cherry`, `red_huckleberry`, `pacific_madrone` feeding `band_tailed_pigeon`, `varied_thrush`, `stellers_jay`, `black_bear` |
| Early flowers feed the pollinators | `bigleaf_maple` flowers in early spring, `salmonberry` flowers feed hummingbirds, `indian_plum` leafs out first of all |
| Alder feeds the soil that feeds the conifers | `red_alder` into `douglas_fir` and `western_hemlock` |

Two of those rows are worth pausing on, because they show the web doing
something a simple chain cannot.

**The surf scoter row is a single animal joining two habitats.** It sits
over the shellfish bed all winter, then switches onto herring eggs
during spring migration. That means one duck's numbers depend on both
the shellfish bed and the herring beach, which is exactly what its record
says: scoter numbers track the health of the beds and the spawn. WDFW
puts wintering scoters on Puget Sound at around 50,000 birds, about 80
percent of them surf scoters, and reports the three-year scoter index
down more than 50 percent since the mid 1990s. A duck is telling you
about a beach.

**The pileated woodpecker row is a species building habitat for others.**
It excavates large nest cavities in snags and large decaying live trees,
and the cavities it abandons become nest sites for owls, ducks and
squirrels. That is the practical argument for leaving a safe dead tree
standing. A snag is not a tidiness problem. It is housing stock.

The dataset's own accounting of the whole community: 99 native, 9
introduced and 14 invasive, out of 122 records.

## Invasive species, as a structural question rather than a moral one

It is easy to talk about invasive plants as villains. It is more useful,
and more accurate, to ask a narrow engineering question: what does this
plant DO to the structure of the place?

Four of them dominate around Silverdale, and each answers that question
differently.

**Himalayan blackberry (`himalayan_blackberry`) takes the ground and
holds it.** Canes 20 to 40 feet long carrying hooked prickles up to 20
millimetres, thickets up to about 13 feet tall and described as
impenetrable. It regenerates from rootstalks, rooting stem tips, and root
and stem fragments, which is why cutting it without follow-up spreads it.
The structural effect is a dense evergreen layer at the exact height at
which conifer seedlings and native shrubs would otherwise be establishing,
on exactly the disturbed ground where succession would otherwise begin.
It does not stop succession by poisoning anything. It stops it by
occupying the seedling layer permanently. Washington lists it as a Class
C noxious weed, and the practical consequence of that listing is that
roadside stands may have been sprayed, which is a foraging problem as
well as an ecological one.

**Scotch broom (`scotch_broom`) rewrites the soil chemistry and then
locks the site.** Like alder, it is a nitrogen fixer, and the Forest
Service reports one measurement, and it is worth knowing where from: in
Monterey pine plantations in New Zealand, broom derived 81 percent of the
nitrogen in its above-ground tissues from the atmosphere, equivalent to 111
kilograms of nitrogen per hectare per year. No equivalent figure is
published for Pacific Northwest broom, so do not set that number beside
alder as though the two had been measured on the same ground. That sounds
like alder's trick, and the
outcome is different, because broom does not hand the site on. Stands
reach a biomass of 44,000 to 50,000 kilograms per hectare in three to
four years. As stands age, the ratio of woody to green material rises and
dead wood accumulates, so a stand of Scotch broom can perpetuate itself
for many years, effectively excluding other vegetation. The Forest
Service calls it a serious pest in logged areas replanted with conifer
seedlings. And the seed bank makes it a long argument: seeds survive in
the soil for at least 5 years and possibly as long as 30, and one
medium-sized shrub can produce several thousand seeds a year. Cutting a
stand once is not a plan.

**English ivy (`english_ivy`) attacks the vertical structure.** On the
ground it forms a dense cover six to eight inches deep that the Forest
Service review describes as an ivy desert, forming near monocultures in
the understorey, suppressing the ground flora and inhibiting regeneration
of native species. Then it climbs. Stems typically reach 90 feet, and
occasionally 300-foot conifers. The Forest Service is careful about what
happens next, and so is this sentence: it records as anecdotal that
climbing ivy covers and kills the supporting branches by blocking
sunlight and that the host tree may eventually die from steady weakening,
and says a tree carrying ivy may be susceptible to windfall in storms.
Reported, not measured. So one plant removes the seedling layer and
destabilises the canopy layer at the same time. It spreads by birds
eating the berries and by stem and root fragments rooting where they
touch soil, which is why yard-waste dumping moves it. In Washington it is
a Class C noxious weed, and four taxa are on the state quarantine list and
may not be transported, bought, sold or distributed.

**Reed canarygrass (`reed_canarygrass`) converts a wetland into a
lawn.** It occupies saturated ground: ditches, dikes, shallow marsh, wet
meadow. The Forest Service describes it making up from over 50 percent to
100 percent of vegetation cover and forming monotypic stands. Its roots
and rhizomes form an almost impenetrable sod, dense rhizomatous mats in
the upper few inches of soil. When it senesces late in the season, the
litter accumulates into thick, impenetrable mats which can exceed 80
percent of total biomass where the stand is not burned. The structural
result is that dense stands prevent the establishment of woody species,
and the review records cases where the grass dominated and precluded
forest development altogether. A wetland that would have become
streamside willow, alder and cedar, the shade and wood that a salmon
creek needs, becomes and stays grass.

Notice what the four have in common. None of them wins by being
poisonous. Each one wins by occupying a structural layer, permanently,
that something else needed in order to get started: the seedling layer,
the open soil, the trunk, the wet ground. That is what "invasive" means
mechanically, and it is why the honest question about a control project
is never "did we kill the plant" but "did something native get the layer
back."

Their own record notes the nuance that gets misquoted, too. Reed
canarygrass origin is genuinely disputed: the Washington noxious weed
board gives Eurasia, while USDA PLANTS records the species as native in
the lower 48. The Forest Service position is that invasive North American
populations are thought to be non-native strains or hybrids, and that
native populations never exposed to that gene flow may no longer occur in
North America. That is native strains possibly swamped, not native
strains never existing. Being precise about that is the difference
between understanding a problem and repeating a slogan.

## Your household is inside the web, not beside it

The most common mistake about ecosystems is treating them as somewhere
else. Here is what a single household on the Kitsap Peninsula changes,
in both directions.

### Runoff, which is the big one

This is the place where the connection is proven rather than argued.

Scholz and colleagues surveyed Seattle-area streams from 2002 and found
adult coho returning to spawn were dying before they could spawn.
Affected fish swam erratically at the surface, gaped, splayed their fins,
lost orientation and equilibrium, and died within hours, with female
carcasses showing over 90 percent egg retention. In one representative
urban stream, Longfellow Creek, premature spawner mortality ran from 60
to 100 percent of every autumn run across 2002 to 2009. In a non-urban
reference stream the comparable rate was under 1 percent.

The pattern across watersheds was the tell, and it comes from a companion
paper rather than from Scholz: Feist and colleagues, analysing land use
across the same watersheds, found coho spawner mortality correlated closely
and positively with the proportion of local roads, impervious surfaces and
commercial property in the basin. Neither team could name the toxicant in
2011 and both said so.

It was named later. USGS describes 6PPD, a chemical used to keep tyres
from degrading and cracking, which reacts with ozone in the air to form
6PPD-quinone. That compound is released from tyres through normal wear,
and once on the roads and in the atmosphere it enters streams through
dust transport, rain and storm runoff, where it kills juvenile coho at
levels found in the environment. For 25 years the autumn die-offs in the
Pacific Northwest were known only as urban runoff mortality syndrome.

Sit with the shape of that. The rain falls on a road. The road is
upstream of a creek. The creek holds coho, and coho hold up eagles,
otters, the forest's nitrogen subsidy and, at the far end, a whale
population that eats their cousins. Where your roof, driveway and street
drain to is an ecological decision, and
[Where Water Goes](where_water_goes.md) is how you find out what yours
is.

### Pets

Two separate problems, and only one of them is the obvious one.

Loss, Will and Marra, working at the Smithsonian Migratory Bird Center
and the US Fish and Wildlife Service, systematically reviewed the
evidence and estimated that free-ranging domestic cats kill 1.3 to 4.0
billion birds and 6.3 to 22.3 billion mammals a year in the United
States, and that un-owned cats rather than owned pets cause the majority
of it. Their conclusion was that free-ranging cats are likely the single
largest source of human-caused mortality for US birds and mammals. The
range is wide because the underlying data are patchy, and the authors are
explicit about the method being a systematic review rather than a census.
Even the bottom of the range is a large number, and the fix for an owned
cat is entirely in the owner's hands.

The other problem is disturbance, which costs an animal energy without
killing it. NOAA notes that disturbance of harbour seal haul-out areas,
particularly nursing mothers and pups, stresses the animals and degrades
the habitat (`harbor_seal`). The record's practical instruction is worth
memorising, because the situation is common and the instinct is wrong: a
pup resting alone on a beach has usually not been abandoned. Do not touch
it, do not put it back in the water, keep dogs and people well back, and
call the stranding network.

### Plantings, for good and ill

English ivy is in Kitsap woodland because it was planted as an ornamental
groundcover and escaped. That is the ill.

The good is equally available and mostly free. The band-tailed pigeon
record (`band_tailed_pigeon`) doubles as a native hedgerow planting list,
because WDFW names its diet: cascara, elderberry, wild cherry,
huckleberry and madrone, all of which grow in central Kitsap and all of
which feed a great deal more than pigeons. Black twinberry
(`black_twinberry`) is described by USDA as a valuable shrub for
streambank erosion control and riparian restoration, with berries eaten
by bears, small mammals, grouse, quail and thrushes and flowers feeding
hummingbirds and butterflies. Oceanspray (`oceanspray`) and salmonberry
(`salmonberry`) hold banks. Bigleaf maple (`bigleaf_maple`) flowers early
enough to be a first food for pollinators.

Planting a hedge is one of the few household actions that adds a
structural layer rather than removing one.

### Attractants, which is where you lose the argument

A bear that finds food at a homestead comes back (`black_bear`). WDFW's
advice is to keep bins with tight lids in a shed, garage or fenced area,
take down bird feeders including hummingbird feeders, and keep fruit
trees picked. The same applies to raccoons (`raccoon`), where securing
rubbish, feeding pets indoors and not leaving pet food out is the
practical control. And on coyotes (`coyote`), the record makes the point
that generalises: secure roofed enclosures and removing attractants work
better than shooting, because a removed coyote is quickly replaced.

That last sentence is the whole discipline in miniature. If you remove an
animal but leave the resource that drew it, you have created a vacancy,
not a solution.

### The garden is not outside the system

A vegetable bed is a patch of early-succession ground that you are
holding at year one on purpose. That is why it needs so much work: you
are fighting the entire sequence described above, every season, and the
alder, blackberry and broom all want the same bare sunlit soil you do.
Your compost heap is the decomposition step, running under your
supervision. Your soil pH is partly a legacy of whatever grew there
before. Your pollinators come out of the hedgerow. Your slugs come out of
the litter layer.

Once you see the garden as a managed point on a successional curve rather
than a separate object on the lawn, most gardening advice starts making
sense for the first time.

## Why this makes you better at everything else

This guide has no hazard attached to it. Nothing here can hurt you, and
nothing here qualifies you to do anything. So it is fair to ask what it
is for.

It is for making every other subject cheaper to learn.

**Foraging** gets easier because ecology tells you where to look and,
more importantly, where something cannot be. A chanterelle grows from
soil on living conifer roots and never on wood, so a ridged orange
mushroom on a stump is a different organism and you can stop there.
Salmonberry ripens first and evergreen huckleberry last, so the year has
an order.

**Growing** gets easier because you stop fighting the site and start
reading it. Alder ground is nitrogen-rich and acid. A wet corner that
grows reed canarygrass is telling you about its water table. The clearing
you are about to make will be full of blackberry in two years unless you
plan for the seedling layer.

**Fishing** gets easier because you know what the fish is doing and when.
Chum spawn low in the creeks and coho push further up and wait for the
first heavy rain. Pinks only run in odd years. A beach full of sand lance
is a beach worth fishing off.

**Building** gets easier because you can see what your structure does to
a layer. A bulkhead removes a spawning band. A hardened bank removes a
kingfisher tunnel. A cleared snag removes an owl's nest site. A driveway
adds impervious surface to a coho creek's watershed. None of that means
do not build. It means know what you are trading, and where the cheap
substitutes are, because a nest box, a rain garden and a gravel drive are
all far less trouble than the thing they compensate for.

And under all of it, one habit: **when you meet something new in a place,
ask what it eats, what eats it, what structural layer it occupies, and
what it would be replaced by.** Four questions, no equipment. They work
on a mushroom, a duck, a weed, a creek and a building lot, and they will
keep working in a place you have never been.

## Sources

Grouped by what kind of authority each one is. United States government
publications are listed first because they are public domain and can be
redistributed with this guide; the rest cannot.

### United States government (public domain)

- USDA Forest Service, Fire Effects Information System. Species review:
  *Alnus rubra* (red alder). Successional status, pioneer establishment
  density, nitrogen accretion of 40 to 300 pounds per acre (45 to 355
  kg/ha) per year, soil nitrogen and humus formation, soil pH under pure
  alder stands versus adjacent conifer, the 25-year and 40-year conifer
  overtopping sequence, stand longevity, shade intolerance and
  self-thinning, and alder early-seral stands as black-tailed deer
  habitat. https://research.fs.usda.gov/feis/species-reviews/alnrub
- USDA Forest Service, Fire Effects Information System. Species review:
  *Tsuga heterophylla* (western hemlock). Shade tolerance ranking
  relative to Pacific yew and Pacific silver fir, climax status, and the
  centuries-without-disturbance outcome. Note that this review does not
  give a figure for light reaching the forest floor, so none is quoted
  here. https://research.fs.usda.gov/feis/species-reviews/tsuhet
- USDA Forest Service, Fire Effects Information System. Species review:
  *Cytisus scoparius* (Scotch broom). Nitrogen fixation at 81 percent of
  above-ground tissue nitrogen and 111 kg N/ha/year, which FEIS scopes to
  Monterey pine plantations in New Zealand and not to this region, stand biomass of
  44,000 to 50,000 kg/ha in three to four years, seed bank persistence of
  5 to 30 years, seed production per shrub, self-perpetuating stands
  excluding other vegetation, and conifer plantation damage.
  https://research.fs.usda.gov/feis/species-reviews/cytspp
- USDA Forest Service, Fire Effects Information System. Species review:
  *Hedera helix* (English ivy). Ground cover depth, the ivy desert and
  understorey monoculture, suppression of native regeneration, climbing
  height, branch shading and host tree decline, windfall susceptibility,
  and spread by birds and by stem and root fragments.
  https://research.fs.usda.gov/feis/species-reviews/hedhel
- USDA Forest Service, Fire Effects Information System. Species review:
  *Phalaris arundinacea* (reed canarygrass). Cover percentages and
  monotypic stands, impenetrable rhizome sod, litter mats exceeding 80
  percent of biomass, prevention of woody species establishment, and the
  native-versus-introduced strain question.
  https://www.fs.usda.gov/database/feis/plants/graminoid/phaaru/all.html
- NOAA Fisheries. Phytoplankton of the Northwest U.S. Shelf Ecosystem
  (phytoplankton responsible for nearly all primary production, the
  definition of primary productivity, phytoplankton as the primary food
  source for zooplankton and filter feeders, the dependence of total
  organisms on primary production, and the size range).
  https://www.fisheries.noaa.gov/west-coast/science-data/phytoplankton-northwest-us-shelf-ecosystem
- National Park Service. Salmon Nutrient Cycling (the definition of
  marine-derived nutrients, transport by mammals, birds and insects
  through carcass dragging and defecation, salmon nutrients found up to
  seven miles from the stream of origin, and the use of stable isotope
  nitrogen-15 as a tracer).
  https://www.nps.gov/teachers/classrooms/salmon-nutrient-cycling.htm
- National Park Service, Olympic National Park. Fungi (saprophytic fungi
  consuming cellulose and lignin, and the mycorrhizal exchange of tree
  carbon for fungal phosphorus and nitrogen).
  https://www.nps.gov/olym/learn/nature/fungi.htm
- United States Geological Survey, Environmental Health Program. 6PPDQ
  (what 6PPD is and why it is in tyres, formation of 6PPD-quinone by
  reaction with ozone, release through normal tyre wear, transport to
  streams by dust, rain and storm runoff, coho sensitivity, and the
  25-year history of urban runoff mortality syndrome).
  https://www.usgs.gov/programs/environmental-health-program/science/6ppdq
- US Fish and Wildlife Service. Threats to Birds: Predators. Used only as
  the agency framing for predation as a bird conservation issue; the
  national estimate quoted in this guide comes from Loss et al. 2013
  below, not from this page, which gives a Wisconsin-specific figure.
  https://www.fws.gov/story/threats-birds-predators
- USDA Forest Service Treesearch. Record for Ben-David, Hanley and
  Schell 1998 (five plant species, 18 transects from stream to upland at
  0 to 1000 m; declining marine nitrogen signature with distance and
  elevation in three of five species; higher values at sites used by
  fish-eating predators; and the authors' own caveat that the importance
  depends on whether nitrogen limits plant growth in that system).
  https://research.fs.usda.gov/treesearch/20049

### Peer-reviewed literature (cited per paper)

- Helfield, J.M. and Naiman, R.J. 2001. Effects of salmon-derived
  nitrogen on riparian forest growth and implications for stream
  productivity. *Ecology* 82(9): 2403 to 2409. Study sites on the
  Kadashan and Indian rivers, Chichagof Island, southeast Alaska; about
  22 to 24 percent of foliar nitrogen from salmon (24 percent Sitka
  spruce, 22 percent devil's club, 22 percent fern, 1 percent red alder);
  basal area growth more than tripled within 25 m of spawning reaches;
  86 years versus 307 years to reach 50 cm diameter at breast height;
  the authors' caveat about unknown timescale; and the riparian feedback
  through shading, filtration and large woody debris. Full text read at
  https://www.biol.wwu.edu/hooper/Helfield&Naiman2001Ecol_Salmon-derivedN&ForestGrowth.pdf
- Reimchen, T.E. and Fox, C.H. 2013. Fine-scale spatiotemporal
  influences of salmon on growth and nitrogen signatures of Sitka spruce
  tree rings. *BMC Ecology* 13: 38. Black bears as the dominant vector,
  eight bears moving 3,100 salmon and 10,700 kg in one year, about 330 g
  of carcass remnants per square metre, carcasses within 50 m, and the
  tree GROWTH response extending at least 90 m into the forest. The paper
  is explicit that its nitrogen signature runs the other way, highest
  beside the spawning area and declining with distance, so 90 m is a
  growth result and not a nitrogen result.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC3850941/
- Paine, R.T. 1966. Food Web Complexity and Species Diversity. *The
  American Naturalist* 100(910): 65 to 75. The Mukkaw Bay experiment and
  the hypothesis quoted at page 65 are cited here through Lafferty and
  Suchanek 2016 below rather than from the original, which was not
  opened for this guide. Paine's coining of "keystone species" is in his
  1969 note in the same journal, not in the 1966 paper.
- Lafferty, K.D. (US Geological Survey, Western Ecological Research
  Center) and Suchanek, T.H. 2016. Revisiting Paine's 1966 Sea Star
  Removal Experiment, the Most-Cited Empirical Article in the American
  Naturalist. *The American Naturalist* 188(4): 365 to 378. DOI
  10.1086/688045. Mukkaw Bay in spring 1963, the crowbar, the 8 m by 2 m
  cleared stretch, the barnacle then mussel sequence, the fall from 15 to
  eight primary space-holding species, mussel beds as three-dimensional
  habitat for over 300 associated species, and the citation analysis
  (22 percent citing the 1966 paper for the keystone concept, 10 percent
  naming sea stars, 5 percent naming mussels).
  https://polydora.github.io/General-ecology/Literature/Paine-1966-Revisit.pdf
- Mills, L.S., Soule, M.E. and Doak, D.F. 1993. The Keystone-Species
  Concept in Ecology and Conservation: Management and policy must
  explicitly consider the complexity of interactions in natural systems.
  *BioScience* 43(4): 219 to 224. Only the title, subtitle and
  publication details were available without a subscription, so this
  guide cites the paper for the existence and direction of the critique
  and quotes nothing from its body.
  https://academic.oup.com/bioscience/article/43/4/219/398794
- Eisenlord, M.E., Groner, M.L., Yoshioka, R.M., Elliott, J., Maynard,
  J., Fradkin, S., Turner, M., Pyne, K., Rivlin, N., van Hooidonk, R. and
  Harvell, C.D. 2016. Ochre star mortality during the 2014 wasting
  disease epizootic: role of population size structure and temperature.
  *Philosophical Transactions of the Royal Society B* 371(1689):
  20150212. 16 sites in the San Juan Islands, South Puget Sound and the
  Washington outer coast, 6,568 stars surveyed from December 2013 to
  July 2015, peak prevalence to 100 percent with a mean of 61 percent,
  populations down 67 percent on average and 80 percent among adults,
  San Juan adults to a quarter of pre-outbreak abundance, and the
  predicted mussel influx.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC4760142/
- Scholz, N.L., Myers, M.S., McCarthy, S.G. and others. 2011. Recurrent
  Die-Offs of Adult Coho Salmon Returning to Spawn in Puget Sound
  Lowland Urban Streams. *PLoS ONE* 6(12): e28013. The symptoms, over 90
  percent egg retention, Longfellow Creek premature spawner mortality of
  60 to 100 percent of each autumn run from 2002 to 2009, under 1 percent
  in the non-urban reference stream. This paper does NOT report the
  land-use correlation; it cites Feist et al. for it, and so does this
  guide.
  https://journals.plos.org/plosone/article?id=10.1371%2Fjournal.pone.0028013
- Feist, B.E., Buhle, E.R., Arnold, P., Davis, J.W. and Scholz, N.L. 2011.
  Landscape ecotoxicology of salmon spawner mortality in urban streams.
  *PLoS ONE* 6(8): e23424. The source of the land-use correlation quoted
  above. Cited here at second hand, through the Scholz 2011 discussion,
  which states the finding and attributes it to this paper; the Feist
  paper itself was not opened for this guide, so no link is given for it:
  that is the point of saying where the claim came from.
- Loss, S.R. (Smithsonian Migratory Bird Center), Will, T. (US Fish and
  Wildlife Service, Division of Migratory Birds) and Marra, P.P. 2013.
  The impact of free-ranging domestic cats on wildlife of the United
  States. *Nature Communications* 4: 1396. DOI 10.1038/ncomms2380. The
  1.3 to 4.0 billion bird and 6.3 to 22.3 billion mammal estimates,
  un-owned cats causing the majority, and the systematic-review method.
  https://www.nature.com/articles/ncomms2380

### University and state bodies (cited as the authority; text written here)

These are not covered by the federal public-domain rule, so the facts are
restated in our own words with the body named.

- Encyclopedia of Puget Sound, published by the Puget Sound Institute at
  the University of Washington Tacoma. Transfer of nutrients in the
  ecosystem. Reports Bilby and colleagues (1996) finding salmon-derived
  nitrogen at 10 to 20 percent of the nitrogen in some fish and
  invertebrate species in a western Washington salmon stream, with higher
  proportions in Alaskan systems.
  https://www.eopugetsound.org/articles/transfer-nutrients-ecosystem
- Washington Department of Fish and Wildlife. Species accounts behind the
  shipped records used above: surf scoter diet and the Puget Sound
  wintering estimate and index decline, pileated woodpecker nesting
  cavities and their reuse, beaver ponds as salmon rearing habitat,
  Pacific herring and surf smelt spawning substrate and the coastal
  squeeze, Pacific sand lance intertidal habitat, black bear and raccoon
  attractant management, band-tailed pigeon diet, and Southern Resident
  killer whale prey. Cited per record in the species data file.
  https://wdfw.wa.gov/
- Washington State Noxious Weed Control Board. Class listings for
  Himalayan blackberry, Scotch broom, English ivy and reed canarygrass,
  and the state quarantine on English ivy. Cited per record in the
  species data file. https://www.nwcb.wa.gov/

### The locale data this guide's worked examples rest on

Every record id in backticks above (`chum_salmon`, `red_alder`,
`surf_scoter`, `pileated_woodpecker` and the rest) is an entry in
`data/locales/silverdale_wa/species.json`, which records its own source
URLs per record and its own licence at the top of the file. Its
provenance is US federal sources plus Washington State agencies restated
in our own words, with nothing taken from GBIF, iNaturalist, Wikipedia or
OpenStreetMap. The record shape is defined in `schemas/locale.toml`. The
seasonal timings quoted here (the chum and coho runs, herring spawn, the
autumn mushroom flush, the berry order, waterfowl arrival) are entries in
`data/locales/silverdale_wa/phenology.json`, which labels each one with
how confident it is. The watershed path from a Silverdale roof to the
Strait of Juan de Fuca is in
`data/locales/silverdale_wa/water.json`.

### Companion guides in this collection

- [The Species Where You Live](the_species_where_you_live.md)
- [What Soil Is](what_soil_is.md)
- [Your Soil, Specifically](your_soil_specifically.md)
- [Where Water Goes](where_water_goes.md)
- [The Year of Wild Food](the_year_of_wild_food.md)
- [The Growing Calendar](the_growing_calendar.md)
- [Reading the Tide](reading_the_tide.md)
- [Your First Compost](your_first_compost.md)
- [Where You Are: Silverdale, Washington](../locale/silverdale_wa.md)
