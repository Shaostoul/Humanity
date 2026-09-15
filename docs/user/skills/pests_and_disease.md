# Pests and Disease

Go out to the bed in the third week of June and look at the kale.

There are holes in it. The outer leaves are lacy, two of them are chewed
back most of the way to the midrib, and on the leaf below there are
small dark grains, like coarse pepper, that were not there last week.
Something is eating your food.

What happens next, for most people, is a trip to a shop and a bottle.
That sequence is what this guide exists to interrupt. Not because the
bottle is always wrong, but because it is almost always the fifth or
sixth thing to try and people reach for it first, before they know what
is eating the plant, before they know whether anything is eating the
plant at all, and before they know whether the damage in front of them
will cost them a single meal.

The word this whole guide turns on is **proportionately**. A response
that is too big costs money, kills the insects that were already solving
the problem for you, and leaves the real cause undiagnosed so that it
comes back. A response that is too small loses a crop you needed. Most
of the skill is in telling those two situations apart, and almost none
of it is in knowing the name of a bug.

## What this guide will and will not do

It will not identify your pest. It cannot. A book cannot look at your
leaf, and a guide written once cannot cover the insects, mites, molds,
bacteria, viruses, nematodes, birds and mammals that might be in your
particular bed in your particular county in a particular week. Any
document that promises otherwise is selling confidence rather than
teaching skill.

What it will do is teach the **decision sequence**: the order of
questions that takes you from "something is wrong" to a response that
fits. That sequence is worth more than a pest list, because you can
apply it to a problem nobody has written about yet, and because the most
common expensive mistake in a home garden is not misidentifying a pest.
It is treating a problem that was never alive.

The worked examples come from `data/plants.csv` and `data/creatures.csv`,
which ship with this project: 189 crop records and 99 living creature
records. Where a claim rests on a record, the record id is given so you
can go and read it. This guide also reports, in its own section near the
end, the places where those two files **cannot** express something this
guide says matters. That is deliberate. A reader who is going to grow
real food should know exactly where the simulation stops.

One boundary, stated at the top rather than buried at the bottom. **This
guide does not tell you what to spray, at what strength, or how much.**
Not for synthetic products, not for products sold as organic, and not
for home remedies. The reason is given in full in its own section, and
it is not squeamishness. In the United States the pesticide label is a
legal document, it is specific to a product and a crop and a place, and
a general-purpose guide that hands you a recipe is handing you something
that may be illegal, ineffective, or harmful on the plant you intend to
use it on. What this guide does instead is teach you to read the label
and tell you who to ask.

## The default answer is "nothing", and that is not laziness

Start with a number, because it reframes everything that follows.

WSU's entomology chapter for Pacific Northwest gardeners puts pest
species at **under one percent** of the roughly one million described
insect species, and says most insects are beneficial in some way. NC
State Extension's insect chapter makes the same claim independently: the
overwhelming majority of insects are harmless or beneficial, and under
one percent are considered pests.

Take the arithmetic at face value. Fewer than one percent of about a
million described species is fewer than ten thousand species worldwide
that ever reach pest status, on every crop, in every climate, across
every continent. The number that are pests of your crops, in your
county, in the month you are standing there, is a very small number. So
when you find an insect on a plant, the prior probability that it is
your problem is low, and the probability that it is a pollinator, a
predator, a decomposer, or a passer-by is high.

That is the real reason "do nothing" is the default. It is not
resignation. It is that most of what you see is either not causing
damage, or is the thing that was going to eat the thing causing damage.

EPA says it plainly in its introduction to integrated pest management,
and because EPA is a federal agency its words are public domain and can
be quoted here directly:

> Not all insects, weeds, and other living organisms require control.
> Many organisms are innocuous, and some are even beneficial. IPM
> programs work to monitor for pests and identify them accurately, so
> that appropriate control decisions can be made in conjunction with
> action thresholds. This monitoring and identification removes the
> possibility that pesticides will be used when they are not really
> needed or that the wrong kind of pesticide will be used.

Two failure modes are named in that last sentence, and they are equally
common: spraying when nothing needed spraying, and spraying the wrong
thing at the right problem. Both are diagnosis failures, not product
failures.

## Step one: is anything actually wrong?

### Damage is not the same as loss

There are three different things people mean by "damage", and they call
for three different responses.

**Cosmetic damage** changes how the plant looks and not what you get
from it. Holes in the outer leaves of a mature cabbage, a few chewed
margins on a squash leaf, a scattering of spots on the lowest leaves of
a tomato in September. If you are eating the plant rather than showing
it, this is a change in appearance and nothing else.

**Damage the plant will outgrow.** Plants have far more leaf than they
strictly need, and they compensate. A vigorous, well-watered, well-fed
plant that loses some leaf area in early season will frequently produce
the same harvest as one that did not, because it puts on replacement
growth. This is not a licence to ignore everything; it matters most on
plants that are growing well and least on plants already stressed by
drought, poor soil, or cold.

**Damage that costs you yield.** The seedling that is severed at ground
level is not going to outgrow anything. The fruit with a grub inside it
is not going to become dinner. The plant with a systemic virus will
never produce what it should. This category is where a response is
justified, and it is a lot smaller than the other two.

The mistake to avoid is treating the first two categories as though they
were the third, which is what happens when you look at a leaf rather
than at a harvest.

### The threshold idea, honestly

Commercial agriculture has a precise version of this. WSU's tree fruit
programme defines the **economic injury level** as the pest density that
causes damage equal in value to the cost of control. That is worth
sitting with, because the definition contains the whole logic: below
that density, the spray costs more than the damage does, so spraying is
a loss even when it works perfectly.

EPA describes the operational form of the same idea, the **action
threshold**:

> Before taking any pest control action, IPM first sets an action
> threshold, a point at which pest populations or environmental
> conditions indicate that pest control action must be taken. Sighting a
> single pest does not always mean control is needed. The level at which
> pests will become an economic threat is critical to guide future pest
> control decisions.

Now the honest part, which is where most home-garden advice goes quiet.
**Your threshold is not a commercial grower's threshold, and in most
cases it is much higher.** Work through why:

- A commercial grower is selling into a market that pays for appearance.
  A blemished fruit can be worth nothing to them and exactly as much as
  a perfect one to you.
- A commercial grower's cost of control is a per-acre cost spread across
  a crop that will be sold. Yours is the full retail price of a bottle
  you will use a quarter of, against a crop with no sale price at all.
- A commercial grower is protecting a livelihood on a scale where a few
  percent matters. You are usually protecting a portion of your food, in
  a garden where the next planting is weeks away.

NC State Extension's IPM chapter makes this distinction explicitly:
thresholds come in an injury form, which is about the health of the
plant, and an aesthetic form, which is about how it looks, and a
homeowner sets their own rather than inheriting a commercial one.

So the threshold in your garden is a judgement, not a published number,
and the right question is not "are there pests" but **"between now and
harvest, will this cost me food I needed?"**

### The one place the threshold gets low, and why

There is an important exception, and it is the clearest illustration in
this whole guide of why the threshold is about consequence rather than
about the sight of a pest.

UMN Extension gives a numeric threshold for cucumber beetles that
differs by crop. For pumpkins and squash the tolerance is relatively
high. For cucumbers and melons, which are susceptible to bacterial wilt,
the threshold drops to **one half of a beetle per plant on seedlings and
one beetle per plant on older plants**. That is a threshold below one
insect. On the same beetle.

The beetle did not get hungrier. What changed is that on those crops the
beetle is not mainly a chewer, it is a **vector**: it carries a
bacterial disease into the plant, and a plant that gets bacterial wilt
is lost regardless of how little leaf was eaten. When the consequence of
a single individual is the whole plant, the threshold collapses toward
zero.

Hold on to that shape. It recurs. Aphids on a healthy established plant
are usually beneath notice; aphids on a crop where they are spreading a
mosaic virus are a different problem with the same insect. The pest is
the same, the threshold is not, and you cannot know which situation you
are in without knowing what the organism does on that crop.

### Ask the question about aphids, because it is the test case

UMN Extension's aphid page says something most garden writing will not:
in most cases aphids cause little or no damage to plants and can be
ignored, and in many cases there are no visible symptoms of aphid
feeding at all.

This is the single most useful sentence in the home-garden pest
literature, because aphids are the most commonly sprayed insect in home
gardens and are, most of the time, not the problem. They are small, 2 to
4 mm long, which UMN gives as one sixteenth to one eighth of an inch,
pear-shaped and soft-bodied, and they cluster on unopened flower buds,
the undersides of young leaves, and developing stems. They are easy to
see, easy to photograph, and easy to feel alarmed about, and none of
those are the same as being damaging.

The section on the response ladder comes back to aphids, because they
are also the textbook case where a broad spray makes the situation
worse. For now, the point is only this: **being visible is not the same
as being a problem, and the visible thing is not always the problem
thing.**

## Step two: is it even alive?

This is the step people skip, and it is the one that costs the most.

### Most plant problems are not pests

It is common to see a figure quoted, something like "seventy percent of
plant problems examined by diagnosticians are non-living in origin". I
could not verify that number on any page I opened for this guide, so it
is not stated here as a fact and you should treat it as unsourced
wherever you meet it. What can be shown is the shape of the thing, from
two directions.

First, from how the diagnostic services are organised. WSU's home-garden
diagnostic resource, Hortsense, carries more than a thousand fact sheets
for Washington gardeners; the counts published for its categories are
421 ornamentals, 152 tree fruits, 63 small fruits, 135 vegetables, 31
lawn and turf, 90 weeds, 5 vertebrates, 85 common problems, 38 natural
enemies and 27 pollinators, which sum to 1,047. What matters here is the
internal structure of the "common problems" category. WSU divides it
four ways: cultural problems, which it describes as related to our
environment and to human actions and activities; plant diseases; insects
and mites; and herbicide damage. Twenty-five of those fact sheets are
cultural and eight are herbicide damage, so **33 of the 85 are about
causes that were never alive**, leaving 52 across diseases and insects
together.

Read the cultural list and you will recognise your own garden in it:
chlorosis, drought damage, fertiliser burn, frost injury, hail damage,
lime-induced chlorosis, marginal leaf necrosis, nutrient deficiency,
oedema, overwatering or poor drainage, girdling and circling roots, poor
pollination, salt damage, sunscald, transplant shock, winter desiccation
and winter injury, among others. Every one of those produces a plant
that looks ill. None of them is a pest, and none of them responds to any
spray.

Second, from the definitions. NC State Extension's chapter on diseases
and disorders defines an **abiotic disorder** as one caused by an
environmental condition, a cultural practice, or chemical exposure, and
notes the property that makes this whole distinction usable: abiotic
disorders are **not contagious**.

### The pattern test

There is a genuinely good diagnostic here, and it works before you know
what organism you are looking at. NC State Extension's diagnostics
chapter gives it as a comparison, and it is the most portable thing in
this guide:

| Points to something living | Points to something non-living |
|---|---|
| Not widespread | Widespread |
| Species specific | Affects several species |
| Hot spots, patches | 100 percent of vulnerable parts affected |
| Progressive with time | Sudden death or decline |

And the rule that follows from it, in NC State's own framing: if the
problem is confined to a single plant family, consider living causes; if
more than one plant family is showing the same symptom in the same
place, a non-living cause is more likely.

Why this works is worth understanding rather than memorising. A living
pathogen or insect has a host range and has to travel. It starts
somewhere and spreads, so it makes edges, patches and gradients, and it
gets worse over days and weeks. A frost, a salt spill, a herbicide
overspray or a drought does not care what family a plant belongs to,
arrives everywhere at once, and produces its full effect in a single
event.

So before anything else, walk the bed and ask four questions:

1. **Is it one species or several?** If your lettuce, your beans and
   your marigolds all have the same symptom, stop looking for an insect.
2. **Is it a patch or the whole bed?** A patch suggests something that
   spread from a point, or a soil condition that varies across the bed.
   Uniform damage across everything suggests weather, water or chemical.
3. **Did it appear overnight or over a fortnight?** Sudden and complete
   points away from living causes.
4. **Does the pattern follow something you did?** The row nearest the
   path, the plants under the drip line of the roof, the end of the bed
   where you emptied a watering can, the side facing the neighbour who
   sprayed their lawn on Saturday.

### The four non-living causes that get blamed on pests

**Water, in both directions.** Too little and too much produce
overlapping symptoms, which is the trap: wilting, yellowing, dropped
leaves and dieback all occur in a drought and in a waterlogged root
zone, because a root sitting in water cannot take up water either. WSU
lists both drought damage and overwatering or poor drainage as distinct
cultural problems. [What Soil Is](what_soil_is.md) explains why the same
watering habit produces a drought in one garden and root rot in another,
and [Your Soil, Specifically](your_soil_specifically.md) is how you find
out which one you have.

**Nutrients, and the diagnostic that comes free with them.** This is the
most useful single trick in plant diagnosis, and it follows from
chemistry rather than from a rule of thumb. Some nutrients are mobile
inside the plant and some are not. When a mobile nutrient runs short,
the plant moves it out of old tissue into new growth, so the **old
leaves** show the deficiency first. When an immobile nutrient runs short,
the plant cannot rob the old leaves, so the **new growth** shows it
first.

WSU's nutrient deficiency fact sheet gives the worked examples:
nitrogen shortage typically shows as a yellowing of **older** leaves;
iron shortage shows on the **new** growth as interveinal chlorosis, that
is, yellowing between the veins while the major veins stay green;
magnesium shortage can also produce interveinal chlorosis; boron
shortage can produce necrotic spotting or flecking.

So "which leaves went yellow first" is a real question with a real
answer, and it costs you nothing to ask. Yellow from the bottom up
points one way, yellow in the new growth at the top points another.

Two honest cautions on that. First, WSU notes that high pH soil binds
iron, so an iron symptom can be a pH problem with the iron present all
along; adding iron to soil whose pH is locking it up is a common and
useless purchase. Second, WSU also notes that root damage, disease and
trunk or branch injuries impede nutrient uptake and distribution, so a
deficiency symptom can be a plumbing failure rather than a supply
failure. The fact sheet's own management advice is to have the soil
tested for pH and nutrients and fertilise accordingly, and its chemical
management recommendation is "none recommended".

**pH.** Not a symptom of its own so much as a cause of other symptoms,
because it decides what the roots can take up out of soil that already
contains the nutrient. The Silverdale locale is a case in point, and
[Your Soil, Specifically](your_soil_specifically.md) covers it.

**Herbicide, including some you did not apply.** WSU keeps eight
separate fact sheets on herbicide damage, covering 2,4-D and triclopyr,
dicamba, dichlobenil, fluazifop, glyphosate and the sulfonylureas,
horticultural spray oil, long-term residual herbicides, and the
triazines. A category that large exists because the damage is common,
because it mimics disease convincingly, and because the source is
frequently not the garden it appears in. Drift from a neighbouring lawn
application, residue in bought compost or manure, or a sprayer that was
previously used for weedkiller will all produce distorted, discoloured
or stunted growth on a plant nobody sprayed.

The pattern test is what catches this one: herbicide drift usually hits
several unrelated species at once, it is often strongest on one side or
one edge, and it arrives after an event rather than spreading from a
point. [The Species Where You Live](the_species_where_you_live.md) makes
a related point from the foraging side, that a roadside stand may have
been sprayed by a crew who left no sign.

### Physiological disorders, which look exactly like disease

Some symptoms are the plant's own physiology going wrong under stress,
with nothing eating it and nothing infecting it.

The canonical one is blossom end rot, and it is already covered properly
by [Your First Tomato](your_first_tomato.md), which makes exactly the
right move: it is not a disease, nothing is eating your plant, and
sprays are not the fix. WSU classes it the same way, as a physiological
problem caused by insufficient calcium reaching the end of the fruit,
with inconsistent soil moisture and high temperatures as frequent
contributing factors, and its chemical management recommendation is
"none recommended".

That last detail is the one to generalise. When a state extension
service's own fact sheet says "none recommended" under chemical
management, it is telling you the category of the problem, not being
cautious. There is nothing to spray because there is nothing alive to
spray at. Among the WSU fact sheets read for this guide, blossom end
rot, nutrient deficiency, mosaic viruses, moles, voles and deer damage
all carry "none recommended" for chemical control.

### A worked example of a misdiagnosis, from the shipped fact sheets

WSU's fact sheet on moles contains one of the most useful sentences in
the whole home-garden literature: moles seldom cause significant damage
to landscape plants, and plant damage attributed to moles is often
actually caused by **voles**.

Look at why the confusion is so reliable. Moles feed primarily on
invertebrates, with earthworms making up much of the diet, plus grubs,
slugs, snails and other insects at various life stages. They are
insectivores. They wreck the surface of a lawn because they tunnel:
permanent tunnels roughly 3 to 12 inches deep, which is about 8 to 30
cm, surface tunnels to about 4 inches or 10 cm leaving a raised ridge,
and conical mounds of pushed-up soil. Highly visible, and not eating
your plants.

Voles are rodents and do eat plants: grasses and forbs in summer, and
roots, bark and bulbs in winter, which is when they girdle young woody
plants. WSU gives the identification as small ears, blunt noses, small
front feet and relatively short tails, about 5 to 8 inches long, which
is roughly 13 to 20 cm, with around sixteen species in the Pacific
Northwest. The damage signature is tiny tooth scars on woody plants.
And, crucially, voles will use the tunnel systems the moles built.

So the visible animal made the visible holes, a different and less
visible animal ate the plants, and the two are found in the same place.
Someone who controls moles has removed an earthworm-eater and left the
plant-eater in possession of a tunnel network. The lesson generalises
well beyond moles: **the conspicuous organism is not automatically the
responsible one.**

Two further things from those fact sheets, both stated as what WSU
publishes rather than as advice from here. WSU's non-chemical options
for voles include hardware cloth barriers using quarter-inch mesh, which
is about 6 mm, buried 24 to 30 inches or roughly 60 to 76 cm deep with
about 6 inches or 15 cm above ground, keeping lawns short, limiting
mulch depth, and traps in runways. And WSU's mole fact sheet notes that
body-gripping traps are illegal in Washington. **Trapping and wildlife
law changes, and it is state law, so confirm the current rule with
Washington Department of Fish and Wildlife before you set anything**;
do not rely on this document's restatement of a fact sheet for a legal
question.

Voles are also, in WSU's own framing, an important food source for hawks
and owls. Both of those exist in the shipped creature data as `hawk` and
`owl`, though nothing in the data connects them to a plant-eating
rodent, because the data has no vole. More on that later.

## Step three: what kind of living thing is it?

If the pattern test points to something alive, the next question is what
kind, because the answer determines everything about the response.

### Read the mouthparts from the damage

Insects damage plants in ways that follow directly from how their mouths
are built, so the damage itself tells you the category even when the
animal has gone.

**Chewing.** Beetles, caterpillars, grasshoppers and their relatives
bite pieces out. WSU's entomology chapter describes chewing damage as
holes or gouges taken out of plant tissue, and lists the more distinctive
forms: hole defoliation, leaf mining, stem boring, and leaf
**skeletonisation**, where only the leaf venation is left behind. NC
State describes the same category as noticeable holes in leaves, wood or
fruit.

Each of those sub-patterns is informative. A leaf **mine** is a pale
winding or blotchy track inside the leaf, made by something living
between the upper and lower leaf surfaces, which is why surface sprays
so often fail against it. **Skeletonisation** means the feeder is
taking the soft tissue and leaving the veins, which tells you about the
size and technique of the feeder. **Shot-holes**, many small round
perforations, are a different signature again, and a fungal leaf spot
whose dead centres have dropped out can imitate them closely, which is
exactly why this is a category question and not a conclusion.

**Piercing and sucking.** Aphids, scale insects, whiteflies, true bugs
and their relatives have a straw-like structure that punctures tissue
and draws out fluid. They make no holes. WSU lists the symptoms as tissue
spotting or stippling, galling, leaf curling and distortion, needle
drop, and catfacing, and notes that sucking insects often leave
predictable symptoms because of toxins or enzymes in their saliva. NC
State's list is stippling, spotting, stunting, yellowing, distorted
growth, and **honeydew**, the sugary excrement of some sucking insects,
which can then grow sooty mold.

The honeydew chain is worth learning as a chain, because you often see
the last link first. Sucking insect feeds, produces honeydew, honeydew
attracts ants and yellowjackets, and a dark fungus called sooty mold
grows on the honeydew. UMN describes exactly this sequence for aphids.
So: **black sooty coating on leaves means a sucking insect above it, and
a stream of ants going up a stem means the same.** The ants are not
eating your plant. Neither is the sooty mold, particularly.

**Rasping.** Thrips are the common case, and they sit between the other
two categories: they scrape the surface open and then drink what comes
out. NC State describes the result as discoloured and distorted flowers
and buds and grey speckled areas on foliage or fruit; the common field
description is irregular silvery streaks or splotches, because the
scraped cells fill with air. Thrips are very small, so the damage is
usually found before the animal is.

### Read the evidence they leave

NC State's insect chapter lists the physical evidence worth looking for
as **frass**, feeding traces, castings, dead bodies and nests. Two of
those are worth expanding.

**Frass** is insect excrement, and it is the single most useful clue on
a chewed plant, because chewers produce a lot of it and it accumulates
directly below where they are feeding. The dark grains on the leaf below
the chewed kale in this guide's opening scene are frass. Fresh frass
means the feeder is still there, probably within a few inches of where
the frass landed. Old, weathered frass with no new damage means the
episode is over and you are looking at a historical record, which is an
entirely different situation and needs no response at all.

**Slime trails.** WSU's slug fact sheet gives the diagnosis as
characteristic slime trails and pretzel-shaped droppings, with the
damage being raggedly chewed older leaves and young tender plants partly
or completely consumed. That combination, ragged chewing with no insect
present and a dried mucus trail catching the light, is about as
diagnostic as garden damage gets.

### Where on the plant, and when

**New growth versus old growth.** Old growth is tougher, often hairier,
and lower in nitrogen. New growth is soft and rich. Many sucking insects
concentrate on it for that reason, which is why UMN describes aphids on
unopened flower buds, undersides of young leaves and developing stems.
So damage concentrated in the newest tissue suggests something that
chose it, and damage on the oldest lowest leaves suggests either a
ground-dwelling feeder that started at the bottom or, very often, a
nutrient or watering problem rather than a pest at all, per the mobility
rule above.

**Whole-plant wilt versus a patch of wilt.** A plant that wilts entire
and does not recover overnight has a problem with its water transport:
either the roots are gone, rotted or eaten, or the vascular tissue is
blocked by a pathogen, or the stem is damaged. Wilt confined to one
branch or one side points to something local, a stem borer, a canker, a
partial root loss on that side. Wilt that appears in the afternoon heat
and recovers by morning is ordinary transpiration outrunning supply and
is a watering and soil question.

The move, when a plant wilts entire, is to **dig one up.** People resist
this, because it destroys a plant. Do it anyway: you have already lost
that plant, and the information is in the root zone. Roots that are
brown, soft and sloughing tell you one story; roots that have been
chewed off tell another; roots with knots or galls on them suggest
nematodes, which NC State describes as tiny roundworms with a needlelike
mouth structure that puncture cells; and roots that look fine send you
back up to the stem.

**Above ground versus below.** Damage you cannot see is the reason
seedlings vanish overnight. Cutworms feed at the soil line and sever
young plants, so the plant is not eaten so much as felled, with the
severed top sometimes still lying beside the stump. That signature,
**a clean severance at ground level with the top left behind and no
other feeding damage**, is close to unambiguous. It also stops mattering
as the season goes on, because stems get too thick to cut, which is why
this is a seedling problem rather than a garden problem.

**When to look at night.** A great deal of garden feeding is nocturnal,
which is why so many people conclude that the damage has no cause. Slugs
feed at night, and WSU's fact sheet says directly that checking plants
at night helps confirm the diagnosis. Cutworms are most active at night
or in low light. So: if there is fresh damage every morning and nothing
to be found by day, **go out after dark with a torch and look at the
undersides of leaves and at the soil surface.** This is the highest-value
five minutes in pest diagnosis, and it costs nothing.

### The identification itself

Once you have a category and, ideally, the animal in a jar, you still
have to name it if the response depends on the name. Three honest
options, in increasing order of reliability:

- **Your extension service's home-garden diagnostic.** For Silverdale
  that is WSU Extension, Kitsap County, and its Hortsense fact sheets,
  which are searchable by host plant and problem and which pair each
  problem with management options that are legal in Washington. This is
  the right first stop because it is filtered to your state.
- **A Master Gardener clinic.** Volunteers trained by the extension
  service, usually free, and used to the local problems.
- **A plant diagnostic laboratory.** WSU runs the Plant and Insect
  Diagnostic Laboratory at Puyallup, which serves home gardeners as well
  as commercial growers and will work from mailed or dropped-off samples
  and, where practical, from clear emailed photographs with a
  description of the problem and the plant's care. **There are
  diagnostic fees**, and WSU states that services are not provided
  without payment of them. For a problem that is going to cost you a
  fruit tree or a bed, this is cheap.

NC State's diagnostics chapter adds a sampling tip that is easy to get
wrong: collect from the **healthy** part of the plant as well as the
damaged part, so that whoever is looking can see what normal is on that
plant.

And if you think what you have found is not native and not usual, that
is a separate reporting path. USDA APHIS asks the public to report what
they see and to contact the local APHIS plant health director, who can
help with identification and explain any restrictions that apply in your
state on moving plants and plant products. Because federal detection of
a new invasive pest is easier and cheaper the earlier it happens, this
is one of the few situations where the correct response to seeing one
insect is to tell somebody.

## Step four: insect, disease, or something else?

If it is a disease rather than an animal, the next split matters
enormously, because only one of the four categories below responds to
the thing most people spray.

### Sign versus symptom

NC State's disease chapter gives the distinction that makes the rest
tractable. A **symptom** is the plant's response, the thing you observe
happening to the plant: the spot, the wilt, the yellowing, the rot. A
**sign** is the causal organism itself, or its structures, actually
visible. The white powder of powdery mildew is a sign. The fuzzy growth
on a late blight lesion in humid weather is a sign. A brown spot is only
a symptom, and a hundred different things produce brown spots.

Signs are worth much more than symptoms, so look for them: hold the leaf
to the light, turn it over, look at the lesion margin with a hand lens.

### The disease triangle

NC State states the requirement precisely: disease occurs only when a
pathogen, a susceptible host and a favourable environment are all
present together, and enough time has elapsed.

This is not academic. It is the whole basis of the response ladder,
because **you can usually move the environment and sometimes change the
host, even when you cannot remove the pathogen**. Spacing plants for
airflow, watering the soil rather than the leaves, choosing a resistant
variety, and planting at a different date are all attacks on the
triangle that involve no product at all.

### Fungi

Fungi lack chlorophyll and cannot make their own food, and NC State
notes they spread by wind, water, insects, soil and people, and that
their spores generally require **free liquid water** to germinate before
they can infect.

That water requirement is the lever. A great deal of practical fungal
disease management is simply denying leaves the hours of wetness that
germination needs: water at the base rather than overhead, water in the
morning rather than the evening, space plants so air moves, prune out
congestion, and stake plants up off wet ground.

**The important exception, which is also the most common garden fungus.**
WSU's powdery mildew fact sheet states that powdery mildews do not
require surface moisture for infection, and that the disease is
consequently often most prevalent in **dry** weather, favouring warm
days and cool nights. So the one fungal disease that almost everyone
gets is the one where "keep the leaves dry" does not apply, and where
people therefore conclude their plant must have something else. WSU's
own non-chemical measures for it are about the other two corners of the
triangle: avoid overfertilising, since that pushes the soft new growth it
prefers, gather and destroy fallen leaves, remove infected leaves and
prune out severely infected shoots, space and prune for air circulation,
and plant tolerant or resistant varieties. WSU notes that effective
fungicides exist but must be registered for the host plant, which is a
point the product section returns to.

### Water molds, which are not fungi

This category exists because two of the most destructive problems in a
wet-climate garden belong to it, and because the distinction changes
which product can possibly work.

UMN Extension states that late blight, *Phytophthora infestans*, is a
**water mold** and not a true fungus, and that water-mold-specific
fungicides are therefore needed. The same page gives the disease's
profile: it favours cool, damp conditions in the range of 60 to 70
degrees F, which is about 16 to 21 degrees C; prolonged hot dry days can
halt its spread; leaf lesions are large brown blotches with a green-grey
edge; stem lesions are firm and dark brown with a rounded edge; tomato
fruit develops firm dark brown circular spots over large parts of the
fruit; and in high humidity a thin powdery white growth appears. The
speed is the alarming part: UMN says it can infect and produce thousands
of sporangia per lesion in under five days, and the spores are easily
airborne.

For a mild, damp maritime climate like Puget Sound, that combination of
preferred temperature range and moisture requirement should tell you
this is a local risk rather than an exotic one, and [The Growing
Calendar](the_growing_calendar.md) is where the weather side of that
lives. Note carefully that the temperature range above is UMN's
statement about the organism, not a claim about how often late blight
occurs in Kitsap County, which is not a number this guide could source.

Damping-off is the other water-mold-adjacent case, and it is already
covered properly by [Starting Seeds](starting_seeds.md), which owns it.
UMN attributes it to the fungi *Rhizoctonia* and *Fusarium* together
with the water mold *Pythium*, and the point relevant here is the risk
profile: UMN says these pathogens thrive in cool wet conditions and that
**any condition that slows plant growth increases damping-off**,
specifically low light, overwatering, high salts from over-fertilising
and cool soil. That is a disease whose main driver is how you are
managing the tray.

### Bacteria

NC State's chapter notes that bacteria cannot penetrate a plant's
cuticle. They get in through **wounds or natural openings**, and they
spread in soil, on insects, in splashing water, on infected seed, and on
pruning tools.

Every one of those is actionable, and together they explain a set of
habits that otherwise look superstitious: do not work among wet plants,
because splashing water moves bacteria and wet foliage is easier to
wound; clean pruning tools between plants; do not save seed from a
diseased plant, which [Saving Your Own Seeds](saving_your_own_seeds.md)
already says; and mulch, because mulch stops rain splashing soil up onto
lower leaves.

The classic bacterial leaf symptom is worth recognising: a **water-
soaked** lesion, meaning it looks darkened and translucent as though
soaked through rather than dried out, which then becomes **angular**
because its spread is limited by the leaf veins it cannot cross. In wet
conditions a sticky ooze of bacteria may form, drying to a crust or
film. A fungal leaf spot, by contrast, is more often round, because
nothing is constraining its edges.

### Viruses

NC State is unambiguous: viral infections of plants are incurable, and
they are systemic, meaning the virus is throughout the plant.

**There is no home cure for a plant virus. There is no commercial cure
either.** Nothing you buy will remove it, and a plant that has it has it
for life. The response is removal, and the reason to remove promptly is
not the plant you are removing, which is already lost, but the ones
around it.

WSU's fact sheet on tomato mosaic viruses shows what management looks
like when a cure is impossible, and its chemical control recommendation
is, predictably, "none recommended". What it recommends instead:
immediately remove and destroy infected plants; plant resistant
varieties; control weeds and related crops that act as alternative
hosts; control aphids, because cucumber mosaic virus can be aphid-
spread; do not handle plants after using tobacco products, because
tobacco mosaic virus and tomato mosaic virus spread on tools, hands and
clothing; and wash hands frequently with soap and water when handling
plants.

Two things in that list deserve emphasis. First, **the aphid entry is
the threshold shift from earlier in this guide**: the aphid that could
be ignored on a healthy plant cannot be ignored when it is carrying a
virus between plants. Second, the handling advice means a virus can be
spread by the gardener, which is a genuinely unintuitive route and the
reason sanitation appears in disease management at all.

Do not compost material you have removed for virus or for a serious
disease. [Your First Compost](your_first_compost.md) already covers why:
a cool pile does not reliably kill disease organisms, and you will
spread them later when you spread the compost.

### And the honest summary of this section

Of the four categories, **fungi and water molds are the ones that
respond to a spray**, and only when it is the right kind of spray, on a
plant it is registered for, applied before or early in the infection
rather than after the damage is done. Bacteria respond poorly and mostly
to sanitation and prevention. Viruses do not respond at all.

Most people, finding spots on a leaf, buy a fungicide. If what they have
is bacterial or viral, that purchase has bought nothing, and they will
conclude the product was weak rather than that the diagnosis was wrong.

## Step five: respond proportionately

Here is the ladder. The order is not invented here: EPA, WSU and NC
State all describe the same sequence, and NC State's IPM chapter states
the priority explicitly as cultural, then mechanical, then biological,
then chemical.

The rule for using it is simple. **Start at the top and stop as soon as
the problem is handled.** Every rung down costs more money, more time,
or more collateral damage than the one above it.

### Rung 0: do nothing, and watch

**Cost:** none, except attention.
**When it is right:** most of the time, which is why it is rung zero and
not rung one.

This is the correct response to cosmetic damage, to damage on a plant
that is growing vigorously, to old damage with no active feeder, to any
insect you have not identified, and to a population that is already
being eaten by something else. It is also correct when you simply do not
know yet, because a week of watching is information and an unnecessary
spray is not.

Watching is an active thing, not a decision to ignore. Look again in
three days. Is the damage progressing or static? Are there more of them
or fewer? Did the aphid colony acquire lady beetles, lacewings, syrphid
larvae, or the tan swollen **mummies** that mean parasitic wasps have
already laid eggs in them? UMN's aphid page names all four of those
predator and parasitoid groups and shows a mummy for exactly this
reason. A colony full of mummies is a colony that is already being
destroyed, and spraying it destroys the wasps along with it.

### Rung 1: change the conditions

**Cost:** your labour, and sometimes next season rather than this one.
**When it is right:** for anything driven by environment, which is most
disease and a great deal of what looks like insect damage.

This is attacking the disease triangle where it is softest. Concretely:

- **Water at the base, not over the leaves,** and earlier in the day, so
  leaves are wet for fewer hours. This is aimed straight at the free-water
  requirement for fungal spore germination.
- **Space plants and prune for airflow.** WSU lists this among its
  non-chemical measures for powdery mildew.
- **Stop overfeeding.** WSU's powdery mildew sheet says to avoid
  overfertilising because it encourages the susceptible new growth, and
  to switch to a slow-release or lower-nitrogen source if needed. Soft
  lush growth is what sucking insects and several diseases prefer.
- **Mulch,** which reduces rain splash carrying soil pathogens onto lower
  leaves, and moderates the soil moisture swings behind blossom end rot.
- **Sanitation.** Remove and destroy infected leaves and fallen material
  rather than leaving it to carry the pathogen to next season. WSU lists
  gathering and destroying fallen leaves and pruning out severely
  infected shoots.
- **Rotate.** WSU Extension recommends a 3 to 4 year rotation cycle for a
  garden, on two grounds: growing the same family in the same place year
  after year gives insect pests a reliable food source, and soil-borne
  diseases build up. WSU groups the vegetable families as crucifers
  including cabbage, broccoli, cauliflower, turnip, radish, kohlrabi and
  mustard; cucurbits including melons, cucumbers and squash; solanaceous
  crops including tomatoes, peppers, eggplant and potatoes; and legumes
  including beans and peas. Note that rotation works against things that
  live in the soil and stay put, and does almost nothing against a pest
  that flies in each year.
- **Choose resistant varieties next time.** This is the highest-leverage
  and slowest-acting rung: it changes the "susceptible host" corner of
  the triangle permanently. WSU's fact sheets name resistant cultivars
  for both blossom end rot and mosaic viruses, and WSU's deer fact sheet
  names less-preferred plants.

There is a tension here worth naming rather than smoothing over. WSU's
slug management includes removing the weeds, debris, rocks and boards
that give slugs shelter. But that same litter layer shelters ground
beetles, which eat slugs, and a lot else besides.
[How an Ecosystem Holds Together](how_an_ecosystem_holds_together.md)
puts it exactly: your pollinators come out of the hedgerow, and your
slugs come out of the litter layer. The resolution is spatial rather
than absolute: clear shelter from the immediate vicinity of vulnerable
seedlings, and keep rough ground elsewhere on the plot. Removing every
refuge on the property removes the predators too, and that is a trade
you will lose.

### Rung 2: physical removal

**Cost:** your time, repeatedly. Nothing else.
**When it is right:** small gardens, visible pests, and anything you can
reach.

Hand-picking is unglamorous and extremely effective at garden scale,
because a home garden is small enough to actually inspect. WSU's slug
recommendations include hand-picking and killing slugs when noticed and
trapping them in sunken containers of stale beer, which is a physical
trap rather than a pesticide. UMN's aphid advice includes knocking them
off with a strong spray of water from a hose, which works because
dislodged aphids largely fail to get back on.

Also in this rung: cutting out and destroying an infected shoot, pulling
and destroying a virus-infected plant, and squashing an egg mass on a
leaf underside before it hatches, which is by a wide margin the cheapest
moment to intervene against any chewer.

The limitation is honest: it does not scale, it has to be repeated, and
it is useless against anything inside the plant, inside the soil, or too
small to see. It is not a solution for a leaf miner or a root problem.

### Rung 3: barriers and timing

**Cost:** materials, and a constraint you have to remember to lift.
**When it is right:** when you know when the pest arrives, or when the
plant is only vulnerable for a window.

This rung is underused in home gardens and is often the best answer,
because an insect that cannot reach the plant does not need to be killed
at all.

**Floating row cover** is the general tool. UMN's guidance for cucumber
beetles is to cover the plants before the beetles arrive and to seal the
edges by burying or weighting them, because an unsealed cover is a
greenhouse for whatever is already underneath it. Row cover works
against a long list of things that must fly or walk in.

**And then the catch, which is the whole reason timing is in the name of
this rung.** UMN says to remove the cover when plants begin to flower,
because most cucurbits need insect pollination and the cover excludes
pollinators as effectively as it excludes pests. Leave it on and you
will trade an insect problem for a crop of unpollinated flowers. For
crops that do not need insect pollination, greens and brassicas among
them, the cover can stay.

UMN's own calendar for putting covers on cucurbits, early to mid-June,
is **Minnesota timing and should not be transplanted to Puget Sound.**
Your dates come from your own frost and season data, which is what
[The Growing Calendar](the_growing_calendar.md) is for.

Note also that row cover appears in this collection already, as frost and
wind protection, in the growing calendar. It is the same material doing
two jobs, and the reason to own some is now doubled.

Other barriers in this rung: a collar pushed into the soil around a
seedling stem against cutworms, which works because the cutworm has to
reach the stem at the soil line; fine mesh netting against small flies,
where mesh size decides whether it works at all; hardware cloth under a
raised bed or around a trunk against rodents, at the mesh and depth
specifications WSU gives; and fencing against deer. WSU's deer
recommendation is a sturdy wooden or chain-link fence at least **seven
feet high**, which is about 2.1 m, optionally with an electrified wire
about a metre outside it. That number is worth stating precisely because
undersized deer fencing is one of the most common wasted expenses in a
rural garden.

**Timing without a barrier** is the free version: planting early or late
to miss a pest's peak, harvesting promptly rather than leaving ripe fruit
sitting, and removing crop residues at the end of the season so nothing
overwinters in them. UMN's advice for spotted wing drosophila, a fly
that lays into intact ripening fruit rather than into rotten fruit, is
largely this: harvest frequently so that ripe fruit is not left standing
in the garden.

### Rung 4: biological controls

**Cost:** usually nothing, if you simply stop killing them.
**When it is right:** always, as a background condition rather than an
intervention.

The most reliable biological control is the one already living in your
garden, and the main thing it needs is to be left alone. WSU's Hortsense
carries 38 fact sheets specifically on **natural enemies**, covering
parasitic flies and wasps, predatory beetles including ground beetles
and several lady beetle groups, predatory bugs including damsel bugs,
big-eyed bugs and minute pirate bugs, predatory flies including hover
flies, lacewings, predatory mites and spiders. That is a larger section
than its entire vertebrate and pollinator sections combined, and it
exists because a home gardener's most important pest-control decision is
usually what **not** to do.

WSU's tree fruit programme states the working principle: natural enemies
are to be conserved as much as possible, and some damage, especially to
foliage, is tolerated. The tolerance is not a concession. It is the
input. A predator population needs prey to persist, so a garden with
zero aphids has no aphid predators in it, and is therefore defenceless
against the next aphid that arrives.

WSU's slug fact sheet names the vertebrate and invertebrate side of the
same idea: encourage birds, garter snakes, frogs, ducks and predacious
ground beetles, **while avoiding broad-spectrum insecticides.** The two
halves of that sentence are one instruction.

Purchased biological controls also exist, and one deserves careful
treatment because it is widely recommended without its caveat.
*Bacillus thuringiensis*, usually sold as Bt, is described by EPA as a
naturally occurring soil bacterium that is toxic to certain pest
insects, used against caterpillars, certain beetles, and mosquitoes and
black flies. It is genuinely selective compared with a broad-spectrum
insecticide.

**But "selective" is not "harmless."** The common garden formulation is
selective for the larvae of moths and butterflies, which means it does
not distinguish between the caterpillar eating your cabbage and the
caterpillar of a butterfly you wanted. EPA's own account of Bt crops
notes the agency was aware of Bt's toxicity to some species of moths and
butterflies and required field studies on the actual risk to
butterflies. The shipped creature data contains `butterfly`, with
`farmland` among its habitats, which is the point exactly: the thing you
would be treating shares a habitat and an order with the thing you want.
Bt is still a good rung-4 tool. It is not a free one, and it is a
pesticide, which means everything in the next section applies to it.

### Rung 5: a product, finally

**Cost:** money, risk to non-target organisms, a legal obligation, and
often the problem getting worse.
**When it is right:** when the rungs above have failed or do not apply,
when you have an actual identification, and when the damage genuinely
threatens a crop you need.

EPA's own placement of this rung, quoted directly because it is public
domain:

> Once monitoring, identification, and action thresholds indicate that
> pest control is required, and preventive methods are no longer
> effective or available, IPM programs then evaluate the proper control
> method both for effectiveness and risk. Effective, less risky pest
> controls are chosen first, including highly targeted chemicals, such
> as pheromones to disrupt pest mating, or mechanical control, such as
> trapping or weeding. If further monitoring, identifications and action
> thresholds indicate that less risky controls are not working, then
> additional pest control methods would be employed, such as targeted
> spraying of pesticides. Broadcast spraying of non-specific pesticides
> is a last resort.

Note the internal ladder inside the last rung: targeted before broad,
and broadcast spraying last of all.

### Why a broad spray can make an aphid problem worse

This is the mechanism that justifies the entire ordering above, so it is
worth stating precisely rather than as a slogan.

WSU's tree fruit programme states it directly: broad-spectrum
insecticides kill most natural enemies, allowing outbreaks of the pests
they would normally control. WSU also describes the historical version of
this, where insects developed resistance and new problems appeared
because natural enemies had been eliminated, trapping growers in
escalating pesticide use.

The mechanics of why it lands harder on the predators than on the pest:

1. **The spray rarely kills all of the pest.** Aphids are on leaf
   undersides, in curled leaves, and in buds. Some survive.
2. **It is much more likely to kill the predators**, which are mobile,
   which range across the whole plant and the whole garden rather than
   hiding in one protected spot, and which are present in far smaller
   numbers to begin with.
3. **The pest rebuilds faster.** Aphids reproduce extremely rapidly, and
   many reproduce without mating. The lady beetle or parasitic wasp
   population has to recolonise from elsewhere and then build up.
4. **So the pest returns to a garden with nothing eating it**, and the
   second population can exceed the first. That is called resurgence, and
   the closely related phenomenon where a previously harmless species
   erupts because its own predators were removed is called a secondary
   pest outbreak.

There is a striking field demonstration of point 4 in the peer-reviewed
literature. Hill, Macfadyen and Nash (PeerJ, 2017) followed a
ground-dwelling invertebrate community through four growing seasons in a
commercial arable field in Victoria, Australia, under organophosphate
applications. Both the pest and the natural enemy communities were
significantly affected. At the doubled rate of chlorpyrifos the number
of beetles that prey on slugs fell, and **slugs ran opposite to the
other pests, increasing in number at the higher rate.** More insecticide,
more slugs.

That study is a broadacre field on another continent, not a Puget Sound
vegetable bed, and it is cited here for the mechanism rather than as a
prediction about your garden. But the mechanism travels, and slugs happen
to be the defining pest of a wet maritime garden, which makes it an
uncomfortably apt example.

UMN's aphid guidance draws the practical conclusion in one sentence:
protect natural enemies by avoiding pesticide applications or using low
risk products, because residual pesticides kill a wide range of insects
including the natural enemies.

### The vacancy principle, which applies to every rung

[How an Ecosystem Holds Together](how_an_ecosystem_holds_together.md)
states a principle in the context of larger animals that is exactly as
true of insects and molds: **if you remove an animal but leave the
resource that drew it, you have created a vacancy, not a solution.**

Every rung on this ladder can be read against that test. Rung 1 removes
the resource or the condition, which is why it lasts. Rungs 2 and 5
remove the organism and leave the resource, which is why they have to be
repeated. Rung 3 leaves both in place and puts a wall between them,
which is why it works reliably but only while the wall is up. Rung 4
maintains a second organism that keeps eating the first.

If you find yourself applying the same response every year on the same
crop, you are refilling a vacancy, and the fix is a rung further up.

## If you do use a product

Read this section before you buy anything, not after.

### The label is the law

In the United States, a pesticide label is not packaging. It is an
enforceable legal document. EPA registers a pesticide product, which is
a licence to market it, only after evaluating scientific data, and EPA
describes the label as translating that evaluation into the conditions,
directions and precautions that define who may use the product and
where, how, how much and how often it may be used.

Every registered product carries a version of this sentence, which is
quoted here from EPA directly:

> It is a violation of Federal law to use this product in a manner
> inconsistent with its labeling.

The governing statute is the Federal Insecticide, Fungicide, and
Rodenticide Act, FIFRA, which EPA describes as the federal statute
governing the registration, distribution, sale and use of pesticides in
the United States, and under which EPA prohibits the use of any
registered pesticide in a manner inconsistent with its labeling. NC
State Extension puts the same point in the plainest available terms:
following the directions on the label is required by law.

What "inconsistent with its labeling" covers in practice, and this is
where home gardeners most often go wrong without realising it:

- **Using it on a plant the label does not list.** This is the big one. A
  fungicide registered for roses is not registered for lettuce, and
  applying it to lettuce is a violation regardless of whether it would
  have worked. WSU's powdery mildew sheet makes the same point from the
  other direction: effective fungicides exist, but the fungicide must be
  registered for the host plant.
- **Using it at a higher rate, or more often, than the label allows.**
- **Ignoring the protective equipment the label specifies.**
- **Ignoring the intervals, which are the subject of the next part.**

### The two intervals, and what they are for

Two numbers on a label exist purely to keep people from being poisoned
by a treated plant, and neither is intuitive.

The **restricted-entry interval**, or REI, is the period after an
application during which people are not supposed to enter the treated
area. It exists because a residue that has dried is not a residue that
is gone.

The **pre-harvest interval**, or PHI, is the minimum time that must pass
between the last application and harvesting the crop. It is set so that
residues on the food have declined to the level the tolerance allows by
the time anyone eats it. This is the number that matters most in a food
garden, and it is the one most likely to be discovered too late by
someone who sprayed a crop that was nearly ready.

WSU states that both are listed on the product label and gives a rule
about their interaction that is easy to get backwards: **the REI
supersedes the PHI.** Their worked example is a product with a 72 hour
REI and a 48 hour PHI, where you must still keep out for 72 hours before
harvesting, because you cannot pick a crop you are not allowed to walk
into.

A note on scope, so nothing is overstated. The formal REI framework
belongs to EPA's Agricultural Worker Protection Standard, which is an
occupational regulation covering agricultural workers and pesticide
handlers on farms, forests, nurseries and greenhouses, with owners and
immediate family on family-owned farms exempt from many of its
requirements. The EPA page describing it does not address home
gardeners. **What binds you as a home gardener is the label of the
product in your hand**, and the intervals printed on it. The point of
knowing where the concept comes from is that the intervals are not
advisory garnish; they exist because somebody measured how long a residue
persists.

### Signal words

NC State Extension summarises the toxicity tiers a label signals:

- **DANGER, POISON** marks extremely toxic compounds, fatal at very low
  doses.
- **WARNING** marks moderately toxic products, in a range NC State gives
  as an oral toxicity of 50 to 500 mg/kg.
- **CAUTION** marks slightly toxic products, above 500 mg/kg.

Those milligram-per-kilogram figures are a laboratory measure of acute
toxicity, and lower numbers mean more toxic, because a smaller dose per
unit of body weight was enough. They say nothing about chronic effects,
about effects on bees, or about effects on the soil, and they are not a
safety ranking for the garden. They are a first filter: a home gardener
has very little business with a DANGER product.

NC State's other handling points are worth carrying: read the label at
every purchase, because instructions, precautions and restrictions may
have changed since last time; always store pesticides in their original
containers, never in unmarked ones and never in containers that held
food or drink; and mix only as much as you can apply in one session,
because the disposal of leftover mixed product is a problem you do not
want.

### "Organic" does not mean non-toxic

This one needs to be blunt, because the marketing is relentless and the
belief is nearly universal.

NC State Extension states it directly: be careful with all pesticides,
because natural products can be as toxic to humans and other organisms,
or even more toxic, than some manufactured ones.

Several things follow:

- **A product sold as organic is still a pesticide.** It is still
  registered, it still has a label with directions and intervals, and
  that label still has the force of law.
- **Natural origin says nothing about toxicity.** The dose and the
  chemistry decide that, not the provenance.
- **"Selective" and "safe" are different claims.** Bt is selective, as
  discussed, and still kills butterfly larvae.
- **Broad-spectrum organic insecticides still destroy natural enemies.**
  The resurgence mechanism described earlier does not check the
  certification of the product that removed the predators.

Homemade remedies deserve the same scepticism plus one more problem:
they have no label, so nobody has established what they do to the plant,
what residue they leave, how long it persists, or what they do to bees.
An untested mixture applied to food is not a cautious choice just
because you made it yourself.

### Why this guide will not give you a recipe

You will notice that nothing above names a product, a concentration, an
amount, or a mixing ratio, for any pest, including the ones with obvious
and widely repeated answers. That is deliberate, and here is the full
reasoning rather than a disclaimer.

**Because the legally correct answer is specific to a product, a crop
and a place, and this document is none of those.** Registration differs
by state, so a product legal in one state may not be registered in
yours. Registration differs by crop, so a rate that is lawful on an
ornamental may be unlawful on a food plant. Formulations differ, so two
bottles with the same active ingredient can require completely different
dilutions, and a number written here would be wrong for at least one of
them.

**Because the label is the legal instrument and a guide cannot
supersede it.** If this document printed a rate and the label said
something different, following this document would be a violation of
federal law. There is no way to write a generally applicable rate that
does not carry that risk.

**Because a recipe skips the step that actually matters.** Everything
useful in this guide is upstream of the product: whether the problem is
alive, what kind of thing it is, and whether it crosses your threshold.
A reader who arrives at a dose without passing through those steps has
been given the one piece of information that is dangerous on its own.

**Because labels change and this file does not.** A document that ships
inside a program and is read years after it was written is precisely the
wrong place to freeze a number that a manufacturer may revise next
season.

So the answer to "what do I spray" is genuinely, not evasively: **read
the label of a product registered for your crop in your state, and if
you want a recommendation rather than a product, ask your extension
service.** For Silverdale that is WSU Extension, Kitsap County, whose
Hortsense fact sheets pair each problem with management options that are
registered in Washington, and which put their non-chemical options first
by design. The slug fact sheet's instruction to select non-chemical
management options as your first choice is not this guide's editorial
position. It is WSU's.

## What the shipped data can and cannot represent

This guide's curriculum topic declares two data files,
`data/plants.csv` and `data/creatures.csv`. Reading them closely enough
to teach from is how the following gaps were found, and they are
reported here rather than quietly worked around, because a reader who
understands where the simulation stops will learn more from it than one
who assumes it is complete.

**What is there.** `data/plants.csv` holds 189 crop records across 26
columns, and it is a genuinely good model of what a plant *needs*:
`water_liters_per_day`, `nutrient_n`, `nutrient_p`, `nutrient_k`,
`ph_min` and `ph_max`, `temp_min_c` and `temp_max_c`, `humidity_min` and
`humidity_max`. Those are, almost exactly, the abiotic causes from step
two of this guide. `tomato` carries `ph_min` 6.0 and `ph_max` 6.8;
`potato` carries 5.0 to 6.5. So the data can already express the
condition that produces a deficiency symptom, which is the majority
cause of what people call disease.

`data/creatures.csv` holds 99 records across 20 columns, including
`diet`, `habitat_biomes`, `hostility` and `ai_behavior`. Several
organisms this guide discusses are present: `bee` and `butterfly` as
pollinators, `spider` as a generalist predator, `ant_worker`,
`earthworm` as a decomposer, `snail` as *Cornu aspersum*, the brown
garden snail, with `farmland` among its habitat biomes, and `owl`,
`hawk` and `fox` as vertebrate predators. `duck` is present and
domesticable, which is notable because ducks appear by name in WSU's
list of slug predators to encourage.

**Gap 1: neither file has any field for a pest or a disease.** This is
the structural one. Across 26 plant columns there is nothing for
susceptibility, resistance, tolerance, or damage, and across 20 creature
columns there is nothing for what a creature damages. A crop cannot be
marked resistant to a disease, which means the single most effective
control on the entire ladder, choosing a resistant variety, cannot be
expressed at all.

**Gap 2: `growth_stages` has no branch for a plant in trouble.**
`tomato` carries `seed:sprout:vegetative:flower:fruit:ripe`. It is a
pipeline with no failure state. There is no stage for damaged, infected,
wilting or dead, so a plant in the simulation either progresses or does
not exist, and the whole diagnostic skill this guide teaches has nothing
to attach to.

**Gap 3: `diet` is too coarse to connect a creature to a crop.** It
takes one of herbivore, carnivore, omnivore, photosynthetic, energy or
none. So `snail` is recorded as a herbivore that lives in farmland, but
nothing in the data says it eats lettuce, and nothing distinguishes it
from `cricket`, also a herbivore in farmland, or from `rabbit`. A field
naming the plant ids a creature feeds on would make `creatures.csv`
capable of expressing a pest relationship at all.

**Gap 4: there is no damage-mode field**, so the central diagnostic of
this guide, chewing versus sucking versus rasping and the different
evidence each leaves, cannot be represented. A player cannot be shown a
stippled leaf rather than a holed one.

**Gap 5: the obvious pests are absent from `creatures.csv`.** There is
no aphid, no slug, no caterpillar of any kind, no cutworm, no wireworm,
no vole and no mole. The absence of the slug is the conspicuous one for
a Puget Sound setting, where it is the defining garden pest, and the
absence of the vole matters because the mole-versus-vole confusion is
one of the best teaching examples available. `snail` is the only record
in the file that is plausibly a crop pest, and its own description,
"Slow-moving gastropod protected by a spiral shell", does not mention
that it eats plants.

**Gap 6: no beneficial predators of pests are represented as such.** The
natural enemy groups WSU keeps 38 fact sheets on, lady beetles, ground
beetles, lacewings, hover flies and parasitic wasps, are absent.
`beetle_stag` is present but is a deadwood species rather than a
predator, `spider` is a generalist, and nothing in the data expresses
that one creature suppresses another. Without that relationship the most
important lesson in this guide, that a broad spray removes the control
you already had, is not expressible in the simulation.

**Gap 7: a defect rather than a gap, in the `marigold` row.** Its
description reads "Bright annual companion plant repelling garden pests",
and its `companion_plants` field lists `tomato:pepper:bean`. The broad
pest-repelling claim is not supported by the sources read for this
guide. What is supported is narrower and almost the opposite in
practice: NC State Extension states that dense planting of certain
marigold varieties for an entire season can help reduce populations of
some root-knot nematode species, and states explicitly that **a few
marigolds mixed in among susceptible plants will not be sufficient.**
The data encodes the interplanting that the source says does not work,
and attaches to it the general claim the source does not make. This is
worth correcting in its own commit with its own reasoning, which is why
it is reported here rather than changed.

None of this makes the data bad. `plants.csv` models growing conditions
well, and the gaps are all in one direction: the files describe
organisms in isolation and not the relationships between them. A pest is
a relationship, not a creature, which is probably why a file of creature
records did not capture it.

## What can go wrong

- **You sprayed and it got worse.** Most likely you removed the natural
  enemies and the pest rebuilt faster than they did. Stop spraying, go
  back to rung 0 and watch for lady beetles, lacewings, hover fly larvae
  and parasitised mummies before doing anything else.
- **You sprayed and nothing happened.** Either the diagnosis was wrong
  in category, most commonly a fungicide applied to a bacterial or viral
  problem, or the cause was never alive at all, or the target was
  somewhere the spray does not reach, inside a leaf mine, inside a stem,
  or in the soil.
- **You treated and it came back next year, identically.** You removed
  the organism and left the resource. Go up the ladder to rung 1 and
  change the conditions, or to rung 3 and exclude it.
- **The whole bed went yellow at once.** Several species at once points
  away from a pest. Check water first, both too little and too much, then
  feeding, then pH, then whether anything was sprayed nearby.
- **Only the new growth is distorted and discoloured.** Consider
  herbicide exposure and an immobile nutrient before considering
  disease. Ask what was sprayed within drift distance, and what was in
  any compost or manure you brought in.
- **Only the old lower leaves are yellow.** That is the mobile-nutrient
  pattern, nitrogen most often, before it is anything else. A soil test
  answers it and a fungicide does not.
- **Seedlings disappeared overnight, cut off at the base.** Cutworm
  signature. Look just under the soil beside the stump, and use collars
  on the replacements.
- **Damage appears every morning with nothing visible by day.** Go out
  after dark with a torch. Look for slime trails and pretzel-shaped
  droppings on the way.
- **A plant wilted entirely and did not recover overnight.** Dig it up
  and look at the roots. You have already lost the plant; do not lose the
  information as well.
- **Ants are streaming up the stem.** They are farming something that
  sucks. Look for the aphids or scale above them, and remember that the
  aphids themselves may still be beneath your threshold.
- **You cannot find anyone who sells what the internet told you to
  buy.** Quite possibly it is not registered for that use in your state.
  Ask the extension service what is.
- **You picked a crop and then read the label.** Check the pre-harvest
  interval on that label now, and remember the REI supersedes the PHI.
  If you are in any doubt about what you have already eaten or served,
  the number to call for a person is Poison Control, not the extension
  service.

## How you know it worked

- You can state, in one sentence, whether the problem in front of you is
  alive or not, and give the pattern evidence for your answer.
- When you see an insect on a plant, your first question is what it is
  doing rather than how to kill it.
- You have gone out at night with a torch at least once.
- You have dug up a wilted plant to look at its roots.
- You can tell a chewing pattern from a sucking pattern from a rasping
  pattern without looking anything up.
- You know which of your crops are in the same family, and you are not
  planting them in the same ground in consecutive years.
- You can say which leaves yellowed first, and you know why that
  question matters.
- You have a row cover, and you know both when to put it on and when it
  has to come off.
- You have left an aphid colony alone for a week and watched what
  arrived to eat it.
- You have read a pesticide label all the way through at least once,
  including the crops it lists, the REI and the PHI, before buying it.
- You know the name and the web address of the extension service for
  your county, and you have used the diagnostic fact sheets at least
  once.
- Your response to "there are holes in the kale" is a question, not a
  purchase.

## Sources

Grouped by what kind of authority each one is. United States government
publications are listed first because they are public domain and can be
redistributed with this guide; the rest cannot, so their facts are
restated here in our own words with the body named.

### United States government (public domain)

- US Environmental Protection Agency. Introduction to Integrated Pest
  Management. The definition of IPM, the four-part structure, and the
  passages quoted directly in this guide under "Set Action Thresholds",
  "Monitor and Identify Pests", "Prevention" and "Control", including
  "Sighting a single pest does not always mean control is needed" and
  "Broadcast spraying of non-specific pesticides is a last resort".
  https://www.epa.gov/ipm/introduction-integrated-pest-management
- US Environmental Protection Agency. Introduction to Pesticide Labels.
  The legal force of the label, the sentence "It is a violation of
  Federal law to use this product in a manner inconsistent with its
  labeling", registration as a licence to market, and the label defining
  who may use a product and where, how, how much and how often.
  https://www.epa.gov/pesticide-labels/introduction-pesticide-labels
- US Environmental Protection Agency. Federal Insecticide, Fungicide,
  and Rodenticide Act (FIFRA) and Federal Facilities. FIFRA as the
  federal statute governing registration, distribution, sale and use of
  pesticides, and the prohibition on using a registered pesticide
  inconsistently with its labeling. This page cites FIFRA sections 13
  and 14 as enforcement authority but does not give a subsection for the
  inconsistent-use prohibition, so no subsection is cited in this guide.
  https://www.epa.gov/enforcement/federal-insecticide-fungicide-and-rodenticide-act-fifra-and-federal-facilities
- US Environmental Protection Agency. Agricultural Worker Protection
  Standard (WPS) Overview. The WPS as an occupational regulation, its
  coverage of agricultural workers and handlers, the requirement that
  employers implement restricted-entry intervals, and the exemption of
  owners and immediate family on family-owned farms from many
  requirements. Note that this page does not define REI in a standalone
  sentence and does not address home gardeners, and this guide says so
  rather than filling the gap.
  https://www.epa.gov/pesticide-worker-safety/agricultural-worker-protection-standard-wps
- US Environmental Protection Agency (archived page). EPA's Regulation
  of Bacillus thuringiensis (Bt) Crops. Bt as a naturally occurring soil
  bacterium toxic to certain pest insects, used against caterpillars,
  certain beetles, mosquitoes and black flies, and the agency's awareness
  of Bt toxicity to some species of moths and butterflies together with
  the field studies it required on butterfly risk.
  https://archive.epa.gov/pesticides/biopesticides/web/html/regofbtcrops.html
- USDA Animal and Plant Health Inspection Service. Hungry Pests, How
  USDA Fights Invasive Pests. The request that the public report what
  they see, and the instruction to contact the local APHIS plant health
  director for help identifying pests, reporting concerns, and
  understanding restrictions on moving plants and plant products.
  https://www.aphis.usda.gov/plant-pests-diseases/hungry-pests/what-to-do

### Peer-reviewed literature (cited per paper)

- Hill, M.P., Macfadyen, S. and Nash, M.A. 2017. Broad spectrum pesticide
  application alters natural enemy communities and may facilitate
  secondary pest outbreaks. *PeerJ* 5: e4179. DOI 10.7717/peerj.4179. A
  commercial arable field near Mortlake, Victoria, Australia, through
  barley, two wheat seasons and canola over four growing seasons from
  2004 to 2007, under the organophosphates chlorpyrifos and methidathion.
  Both pest and natural enemy invertebrate communities were significantly
  affected; at the doubled chlorpyrifos rate the number of slug-predating
  beetles fell and slugs increased in number, running opposite to the
  other pests. Cited in this guide for the mechanism only, not as a
  prediction about a home garden on a different continent.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC5740959/

### University and state bodies (cited as the authority; text written here)

These are not covered by the federal public-domain rule, so the facts
are restated in our own words with the body named. For this guide's
canonical locale, Silverdale in Kitsap County, the `state-extension`
role resolves to WSU Extension, Kitsap County.

- Washington State University, Hortsense. The home-garden diagnostic
  resource itself: more than a thousand IPM-based fact sheets, the
  category counts quoted and summed in this guide, and the division of
  "common problems" into cultural problems, plant diseases, insects and
  mites, and herbicide damage. https://hortsense.cahnrs.wsu.edu/
- Washington State University, Hortsense. Common Problems, Cultural. The
  list of 25 cultural fact sheets named in this guide.
  https://hortsense.cahnrs.wsu.edu/common-problems/common-problems-cultural/
- Washington State University, Hortsense. Common Problems, Herbicide
  Damage. The list of 8 herbicide fact sheets. Note that this index page
  does not itself describe herbicide symptoms, so no symptom description
  is attributed to it here.
  https://hortsense.cahnrs.wsu.edu/common-problems/common-problems-herbicide-damage/
- Washington State University, Hortsense. Common Cultural: Nutrient
  deficiency. Nitrogen chlorosis on older leaves, iron interveinal
  chlorosis on new growth, magnesium interveinal chlorosis, boron
  necrotic spotting, high pH binding iron, root and trunk damage
  impeding uptake, the soil test recommendation, and "none recommended"
  for chemical management.
  https://hortsense.cahnrs.wsu.edu/fact-sheet/common-cultural-nutrient-deficiency/
- Washington State University, Hortsense. Tomato: Blossom-end rot.
  Blossom end rot as a physiological problem rather than a disease,
  insufficient calcium at the fruit end, inconsistent soil moisture and
  high temperatures as contributing factors, resistant cultivars, and
  "none recommended" for chemical management.
  https://hortsense.cahnrs.wsu.edu/fact-sheet/tomato-blossom-end-rot/
- Washington State University, Hortsense. Tomato: Mosaic viruses. TMV,
  ToMV and CMV symptoms, spread by tools, hands and clothing, aphid
  spread of CMV, immediate removal and destruction of infected plants,
  resistant cultivars, the tobacco handling warning, handwashing, and
  "none recommended" for chemical management.
  https://hortsense.cahnrs.wsu.edu/fact-sheet/tomato-mosaic-viruses/
- Washington State University, Hortsense. Common Diseases: Powdery
  mildew. White powdery growth as the sign, infection without surface
  moisture and prevalence in dry weather with warm days and cool nights,
  the non-chemical measures listed in this guide, and the requirement
  that any fungicide be registered for the host plant.
  https://hortsense.cahnrs.wsu.edu/fact-sheet/common-diseases-powdery-mildew/
- Washington State University, Hortsense. Common Insects and Mites:
  Slugs. Raggedly chewed older leaves, young plants partly or completely
  consumed, slime trails and pretzel-shaped droppings, night inspection,
  "select non-chemical management options as your first choice",
  clearing shelter, hand-picking, beer traps, encouraging birds, garter
  snakes, frogs, ducks and predacious ground beetles while avoiding
  broad-spectrum insecticides, and the requirement that any bait be
  labelled for the host or site.
  https://hortsense.cahnrs.wsu.edu/fact-sheet/common-insects-mites-slugs/
- Washington State University, Hortsense. Natural Enemies. The 38 fact
  sheets and the groups they cover: parasitic flies, parasitic wasps,
  predatory beetles, predatory bugs, predatory flies, stinging wasps,
  lacewings, predatory mites, spiders and others.
  https://hortsense.cahnrs.wsu.edu/natural-enemies/
- Washington State University, Hortsense. Vertebrates index, and the
  fact sheets on moles, voles and deer damage. Moles as invertebrate
  feeders whose plant damage is often actually caused by voles, tunnel
  and mound dimensions, hardware cloth barriers, and the statement that
  body-gripping traps are illegal in Washington; vole identification,
  seasonal diet, tooth-scar damage, the roughly sixteen Pacific Northwest
  species, their role as prey for hawks and owls, and the hardware cloth
  mesh and burial specifications; deer browse on new shoots and tender
  leaves leaving only petioles, preferred and less-preferred plants, and
  the seven-foot fence. All three carry "none recommended" for chemical
  management.
  https://hortsense.cahnrs.wsu.edu/vertebrates/
  https://hortsense.cahnrs.wsu.edu/fact-sheet/vertebrate-moles/
  https://hortsense.cahnrs.wsu.edu/fact-sheet/voles/
  https://hortsense.cahnrs.wsu.edu/fact-sheet/deer-damage/
- Washington State University Extension. The Pacific Northwest
  Gardener's Handbook, Chapter 15: Entomology. Under one percent of the
  roughly one million known insect species reaching pest status, chewing
  damage as holes and gouges with defoliation, leaf mining, stem boring
  and skeletonisation, and sucking damage as spotting or stippling,
  galling, leaf curling and distortion, needle drop and catfacing.
  https://extension.wsu.edu/pnw-gardeners-handbook/chapter-15-entomology/
- Washington State University Tree Fruit. IPM Overview. IPM as a
  philosophy founded on ecological principles, the economic injury level
  defined as the pest density that causes damage equal in value to the
  cost of control, the conservation of natural enemies with some foliage
  damage tolerated, and broad-spectrum insecticides killing most natural
  enemies and allowing outbreaks of pests they would normally control.
  https://treefruit.wsu.edu/crop-protection/opm/ipm-overview
- Washington State University Crop Protection Guide. Entry and
  Preharvest Intervals. Both intervals being listed on the product
  label, and the rule that the REI supersedes the PHI, with the 72 hour
  versus 48 hour worked example. Note that this page does not define
  either term in a standalone sentence, so the definitions given in this
  guide are written here from the label context rather than quoted.
  https://cpg.treefruit.wsu.edu/pesticide-safety/entry-and-preharvest-intervals/
- Washington State University Extension. Crop Rotation (Yakima County).
  The 3 to 4 year rotation cycle, the reasoning about a reliable food
  source for pests and the build-up of soil-borne disease, and the
  crucifer, cucurbit, solanaceous and legume family groupings.
  https://extension.wsu.edu/yakima/2025/03/22/crop-rotation/
- Washington State University Puyallup, Plant and Insect Diagnostic
  Laboratory. The service, the clientele including home gardeners,
  submission by mail or drop box, the acceptance of clear emailed
  photographs with a description where practical, and the statement that
  services are not provided without payment of diagnostic fees.
  https://puyallup.wsu.edu/plantclinic/
- Washington State University Integrated Pest Management. Home and
  Garden. Used only to establish that Hortsense is WSU's home-garden
  fact sheet resource and Pestsense its indoor equivalent. This landing
  page does not itself define IPM or describe thresholds, and nothing
  else is attributed to it. https://ipm.wsu.edu/communities/home-garden/
- NC State Extension. Extension Gardener Handbook, Chapter 4: Insects.
  Under one percent of insects considered pests with the majority
  harmless or beneficial, chewing mouthparts leaving holes in leaves,
  wood or fruit, piercing-sucking mouthparts and the stippling,
  spotting, stunting, yellowing, distorted growth and honeydew leading
  to sooty mold, rasping-sucking thrips producing discoloured distorted
  flowers and grey speckled areas, frass as excrement, the evidence list
  of frass, feeding traces, castings, dead bodies and nests, and the
  instruction to accurately identify pest and host.
  https://content.ces.ncsu.edu/extension-gardener-handbook/4-insects
- NC State Extension. Extension Gardener Handbook, Chapter 5: Diseases
  and Disorders. Sign versus symptom, the disease triangle requiring
  pathogen, susceptible host, favourable environment and elapsed time,
  fungi lacking chlorophyll and requiring free liquid water for spore
  germination, bacteria being unable to penetrate the cuticle and
  entering through wounds and natural openings and spreading on soil,
  insects, splashing water, infected seed and pruning tools, viral
  infections being incurable and systemic, nematodes as tiny roundworms
  with a needlelike mouth structure, and abiotic disorders as caused by
  environmental condition, cultural practice or chemical exposure and
  being non-contagious.
  https://content.ces.ncsu.edu/extension-gardener-handbook/5-diseases-and-disorders
- NC State Extension. Extension Gardener Handbook, Chapter 7:
  Diagnostics. Abiotic factors including sunlight, temperature, wind and
  precipitation; the living versus nonliving comparison reproduced as a
  table in this guide (not widespread versus widespread, species specific
  versus several species, hot spots versus 100 percent of vulnerable
  parts, progressive versus sudden); the single-plant-family versus
  multiple-family rule; and the instruction to sample healthy tissue
  alongside damaged tissue.
  https://content.ces.ncsu.edu/extension-gardener-handbook/7-diagnostics
- NC State Extension. Extension Gardener Handbook, Chapter 8: Integrated
  Pest Management. "Integrated" as meaning all control measures are
  considered and used as appropriate, the five-step process, a threshold
  as the point at which action should be taken, the distinction between
  injury and aesthetic thresholds with homeowners setting their own
  rather than inheriting a commercial economic threshold, the ordering
  of cultural, mechanical, biological and chemical control, and the
  statement that the only way to effectively combat a problem is to
  diagnose it properly.
  https://content.ces.ncsu.edu/extension-gardener-handbook/8-integrated-pest-management-ipm
- NC State Extension. Extension Gardener Handbook, Appendix B:
  Pesticides and Pesticide Safety. Natural products being potentially as
  toxic or more toxic than manufactured ones, the DANGER/POISON, WARNING
  and CAUTION signal words with the 50 to 500 mg/kg and above 500 mg/kg
  ranges, following label directions being required by law, reading the
  labeling at each purchase, storing pesticides in original containers,
  and mixing only what can be applied in one session.
  https://content.ces.ncsu.edu/extension-gardener-handbook/appendix-b-pesticide-safety
- NC State Extension. Management of Root-Knot Nematodes in Bedding
  Plants. Dense planting of certain marigold varieties for an entire
  season reducing populations of some root-knot nematode species, and
  the explicit statement that a few marigolds mixed in among susceptible
  plants will not be sufficient. This is the basis for the `marigold`
  data defect reported above.
  https://content.ces.ncsu.edu/management-of-root-knot-nematodes-in-bedding-plants
- University of Minnesota Extension. Aphids in home yards and gardens.
  Size and appearance, feeding sites on buds, young leaf undersides and
  developing stems, honeydew attracting ants and yellowjackets, sooty
  mold as a fungus growing on honeydew, the natural enemy list of lady
  beetles, lacewings, syrphid fly larvae and parasitic wasps, the aphid
  mummy, the statement that aphids usually cause little or no damage and
  can be ignored, dislodging with a strong spray of water, and the
  warning that residual pesticides kill natural enemies along with
  everything else.
  https://extension.umn.edu/yard-and-garden-insects/aphids
- University of Minnesota Extension. Late blight of tomato and potato.
  *Phytophthora infestans* as a water mold and not a true fungus with
  the consequence for fungicide choice, the 60 to 70 degrees F damp
  preference, hot dry days halting spread, the leaf, stem and fruit
  symptoms, the white growth under high humidity, thousands of
  sporangia per lesion in under five days, airborne spread, and removal
  or burial of plants at the end of the season.
  https://extension.umn.edu/disease-management/late-blight
- University of Minnesota Extension. How to prevent seedling damping
  off. *Rhizoctonia* and *Fusarium* as fungi with *Pythium* as a water
  mold, contaminated pots, tools and media, the water-soaked mushy
  cotyledons and stems, and the risk factors of cool wet conditions, low
  light, overwatering, high salts from over-fertilising and cool soil,
  with the general rule that any condition slowing plant growth
  increases damping off. Note that this page does not discuss fungicides
  or whether affected seedlings can be saved, and nothing on those
  points is attributed to it.
  https://extension.umn.edu/solve-problem/how-prevent-seedling-damping
- University of Minnesota Extension. Cucumber beetles. Row cover use
  with sealed buried or weighted edges, removal at flowering because
  most cucurbits require insect pollination, and the differential
  threshold of one half beetle per plant on seedlings and one per plant
  on older plants for cucumbers and melons because of their
  susceptibility to bacterial wilt. The early to mid-June covering date
  on that page is Minnesota timing and is identified as such in this
  guide rather than transplanted.
  https://extension.umn.edu/yard-and-garden-insects/cucumber-beetles
- University of Minnesota Extension. Spotted wing drosophila. The wing
  spot on males and the serrated ovipositor on females, the host list of
  cane berries, blueberries, strawberries and wine grapes, egg-laying
  into intact ripening rather than rotted fruit, brown sunken areas from
  larval feeding, and frequent harvest as the primary garden response.
  The Minnesota seasonal timing on that page is not applied to Puget
  Sound here.
  https://extension.umn.edu/garden-and-home/yard-and-garden/yard-and-garden-insects/spotted-wing-drosophila

### What could not be sourced, and what was done instead

- **The frequently quoted figure that about 70 percent of plant problems
  seen by diagnosticians are abiotic.** It appears in search results
  attributed to extension material, but it is on neither of the two NC
  State Extension chapters that would most obviously carry it (Chapter 5
  and Chapter 7), both of which were opened and read for this guide. The
  WSU Puyallup "Fundamentals of Plant Pathology" PDF that may contain it
  would not decode to readable text through either tool available here.
  **No percentage is stated anywhere in this guide**, and the section
  that would have used it says explicitly that the figure is unverified.
- **WSU Extension publication C187, "Row Covers", and the WSU home
  garden "Crop Rotation in the Home Garden" PDF.** Both fetched as
  binary PDFs that would not decode. Nothing is cited from either; the
  row cover and rotation facts in this guide come from the UMN cucumber
  beetle page and the WSU Yakima County crop rotation page respectively,
  both of which were opened and read.
- **A locale-specific statement about how often late blight actually
  occurs in Kitsap County.** The temperature and moisture preferences
  quoted are UMN's description of the organism. No source opened here
  gives a local incidence figure, so none is given.
- **Current Washington law on trapping.** WSU's mole fact sheet states
  that body-gripping traps are illegal in Washington. That is restated
  here as what the fact sheet says, with an instruction to confirm the
  current rule with Washington Department of Fish and Wildlife, because
  a fact sheet is not a statute and this guide does not attempt to state
  wildlife law.

### The simulation data this guide's worked examples rest on

- `data/plants.csv`, 189 crop records across 26 columns, and
  `data/creatures.csv`, 99 creature records across 20 columns. Record
  ids referenced above (`tomato`, `potato`, `marigold`, `snail`,
  `butterfly`, `bee`, `spider`, `ant_worker`, `earthworm`, `duck`,
  `owl`, `hawk`, `fox`, `cricket`, `rabbit`, `beetle_stag`) are entries
  in those files. The gaps and the one defect reported in this guide
  were found by reading them against the sources above.

### Companion guides in this collection

- [What Soil Is](what_soil_is.md)
- [Your Soil, Specifically](your_soil_specifically.md)
- [The Growing Calendar](the_growing_calendar.md)
- [Starting Seeds](starting_seeds.md)
- [Your First Tomato](your_first_tomato.md)
- [Growing Food You Can Live On](growing_food_you_can_live_on.md)
- [Keeping What You Grew](keeping_what_you_grew.md)
- [Saving Your Own Seeds](saving_your_own_seeds.md)
- [Your First Compost](your_first_compost.md)
- [How an Ecosystem Holds Together](how_an_ecosystem_holds_together.md)
- [The Species Where You Live](the_species_where_you_live.md)
- [Where You Are: Silverdale, Washington](../locale/silverdale_wa.md)
