# The Species Where You Live

A field guide is a book about a region. The question you actually have
is about a county, a beach, a season, and a date. Those are not the same
question, and the gap between them is where people get hurt.

Open a guide to the plants of the Pacific Northwest and look up
blackberry. It will tell you the fruit is edible, which is true. It will
not tell you that in Washington the Himalayan blackberry is a listed
noxious weed, that counties run control programmes against it, and that
the roadside thicket you were about to pick from may have been sprayed
this season by a crew who left no sign. Both facts are about the same
plant. Only one of them is in the book, and the other one is the one
that matters on the day.

That is not a criticism of field guides. It is a description of what a
field guide is for. A guide identifies. It cannot know your jurisdiction,
your beach, your closure status or your month, because it was printed
once for a whole region and those things change by county and by day.

So this guide is not a list of species. It is about how to learn the
living things around you: what questions to ask, in what order, which
authorities answer them, and how to read an entry about a plant or a fish
without misreading the silences in it.

The worked examples come from `data/locales/silverdale_wa/species.json`,
which ships with this project: 125 plants, fungi, mammals, birds, fish
and shellfish of one real place on Puget Sound, each carrying its legal
status *here*, its edibility, its toxicity, and what it gets confused
with. That file is a reference. This document is the teacher. The place
is one place; the method is the part that travels.

One boundary, stated at the top rather than buried at the bottom. **This
is not a foraging guide and it does not qualify you to eat anything.**
There is a separate set of guides for that, and they are separate on
purpose. See the last section.

## Why "is this edible" has no general answer

The honest answer to "is this edible" is almost always "it depends where
you are standing, what month it is, and who owns the ground". Four worked
cases, all from the shipped Silverdale data, all true simultaneously.

**The same plant is a crop and a weed.** Himalayan blackberry
(*Rubus armeniacus*) is a Washington State Class C noxious weed, listed
in 2009, and USDA NRCS records that its fruits are highly edible and
commonly collected by berry pickers. Both are correct. The consequence is
practical rather than philosophical: because it is a control target,
assume any stand along a road, ditch or utility corridor may have been
treated with herbicide, and pick somewhere else. The legal status changed
where you may pick, not whether the fruit is food.

**Two animals in the same yard, opposite answers.** The Douglas squirrel
is named explicitly in Washington's protected wildlife rule, WAC
220-200-100, whose title is that protected wildlife shall not be hunted
or fished. It appears nowhere in the game animal or furbearer lists, so
there is no season and no licence that would authorise taking one. The
eastern gray squirrel on the same fence is introduced and unclassified.
One squirrel, protected. The other squirrel, not. Nothing about either
animal tells you which is which except knowing which is which.

**The same genus, the same water, the same day, different rules.** Puget
Sound Chinook salmon is listed as threatened under the federal Endangered
Species Act; NOAA Fisheries lists the evolutionarily significant unit
covering every stream that drains into Puget Sound from the Elwha
eastward, which includes the creeks around Dyes Inlet. Coho in the same
water is not ESA listed at all: the four listed coho units are all
outside Puget Sound. So in Marine Area 10, there are stretches of the
year when a coho may be kept and a wild Chinook caught on the same line,
in the same hour, must go back. A guide that says "salmon are edible" has
told you nothing you can act on.

**The beach is open and the shellfish are closed.** This is the one that
surprises people most, because it sounds like a contradiction. Three
different authorities have to say yes before you may take a clam in
Washington, and they are independent of each other: you need a licence
that includes shellfish harvest, the state fish and wildlife agency must
have that specific beach open within its size and bag limits, and the
state health department's biotoxin status for that beach must be open on
the day. Paralytic shellfish poison is produced by algae, not by the
beach, and Washington DOH is explicit that cooking and freezing do not
destroy it because the toxins are part of the shellfish meat. There is
nothing to see, smell or taste. A beach can be open for recreation, open
by season, and closed for biotoxins, all at once, and the third one is
the one that puts people in hospital.

**Edible and still not lawful to take.** The Olympia oyster is
Washington's only native oyster and is perfectly good food. Wild stocks
in Puget Sound are not currently open for recreational or commercial
harvest, and by 2012 only about 5 percent of the historical oyster bed
habitat remained. Farmed ones are sold. Edibility and legality are
different axes, and a guide that only carries the first one is missing
half the answer.

None of that fits in a regional book. All of it fits in a local list.

## The four questions, in order

For any living thing you meet, there are four questions, and the order
is not arbitrary. Each one can stop you.

**1. What is it?** Not what it looks like. What is it, to a name that
somebody else could check.

**2. Is it protected?** Legal status is not a property of the organism.
It is a property of the organism *in this jurisdiction*, and it can flip
across a county line, a shoreline, or a fish's genetic lineage.

**3. Will it hurt me?** Toxicity, but also the things that are not
toxicity: contamination, accumulated pollutants, herbicide, prions,
parasites, sap that burns in sunlight, dust that causes dermatitis.

**4. What else could this be?**

Almost everybody asks the first three and skips the fourth. It is the one
that hurts them.

Here is why. Questions 1 through 3 are answered by matching. You look at
a thing, you look at a description, and you check off features until you
are satisfied. That process feels like identification and it is not. It
is confirmation, and confirmation gets stronger the more features you
check, which is exactly backwards: a plant with nine matching features
and one deadly twin is not nine-tenths safe.

Question 4 is answered by disproving. You are not asking "does this
match". You are asking "what is everything else this could be, and have I
ruled each of them out". That is a harder question, it takes longer, and
it frequently ends in "I cannot rule that one out", which is the correct
and useful answer.

The shipped species file was built around this. Every single one of its
125 records carries a lookalike list, and the file says why: an empty
lookalike list on something you might eat is the defect that hurts
people, and it was the most common problem found when the records were
checked. When each plant and fungus record was put to an adversarial
verifier whose job was to refute it, 48 of 60 were refuted, and almost
all the refutations were on the toxicity and lookalike fields. The
edible-plant half of a guide is the easy half. The "what else could this
be" half is where the work is.

## A common name is not an answer, a binomial is

The single most useful mechanical habit in this whole subject is to
refuse to work in common names.

Common names are local, ambiguous, and reused. They are not wrong
exactly; they are just not checkable. Here is a live demonstration, run
against the Integrated Taxonomic Information System (ITIS), which is
hosted by USGS with the Smithsonian and describes itself as providing
authoritative taxonomic information on plants, animals, fungi and
microbes of North America and the world.

Search ITIS for the English word **hemlock** and it hands back, among
others:

| The name "hemlock" is attached to | ITIS TSN | What that actually is |
|---|---|---|
| hemlock | 183396 | *Tsuga*, the conifer genus |
| hemlock | 183421 | *Pseudotsuga macrocarpa*, a Douglas-fir relative |
| poison hemlock | 29473 | *Conium maculatum* L. |
| water hemlock | 29456 | *Cicuta maculata* L. |

One English word, sitting on a forest tree whose needle tips are steeped
as tea, and on two plants that kill you. The Silverdale record for
western hemlock (*Tsuga heterophylla*, ITIS TSN 183400, author (Raf.)
Sarg.) carries both deadly namesakes as lookalike entries for exactly
this reason, and its practical rule is the right shape: if something
called hemlock is not a tree, it is not the one you can make tea from.

Now the reverse failure. Search ITIS for *Polygonum cuspidatum*, which is
the name a great many older books and websites still use for Japanese
knotweed. ITIS returns TSN 20889 with a taxon usage rating of **not
accepted** and an unaccept reason of **synonym**, and points you at the
accepted name, *Fallopia japonica* var. *japonica*, TSN 823876. Attached
to that one serial number are three English common names: Japanese
knotweed, fleeceflower, and Mexican bamboo.

So: one plant, three English names, at least two Latin names, one of
which is retired. If you write down "fleeceflower" you have recorded
nothing anybody can verify. If you write down *Fallopia japonica* with
its authority, you have recorded a claim that a stranger in another
country can check against a database, and which resolves cleanly even if
they only know it by an old name.

That is the whole argument for the binomial. It is not scientific
formality. It is the difference between a note you can act on and a note
you cannot.

Two practical habits follow:

- **Record the binomial with its authority string.** "(Raf.) Sarg." after
  *Tsuga heterophylla* is not decoration; it identifies which publication
  established the name, which is how two identical-looking names get told
  apart.
- **Expect synonyms and do not panic at them.** Authorities genuinely
  disagree. The shipped knotweed record notes that USDA and ITIS accept
  *Fallopia japonica* while sources following Kew accept *Reynoutria
  japonica*, and that nobody accepts *Polygonum cuspidatum*. A live
  disagreement between two respectable authorities is a normal condition,
  not a sign that one of them is broken.

## The tools that actually answer the first question

These are the federal ones, which matters for two reasons: they are free,
and works of the US federal government are in the public domain, so what
they publish can be quoted and redistributed rather than merely linked.

**ITIS, for the name.** `https://www.itis.gov/`. Give it a scientific
name or a common name and it returns the accepted binomial, its
authority, its Taxonomic Serial Number, its synonyms, and its position in
the hierarchy. This is the first stop, not the last, because everything
else you look up will want the accepted name as its key.

**USDA PLANTS, for plants.** `https://plants.usda.gov/`. Per plant it
gives the accepted name and symbol, synonyms, growth habit and duration,
and a native-or-introduced status for each of the lower 48, Alaska,
Hawaii, Canada and Puerto Rico. Query salal (*Gaultheria shallon*, symbol
GASH) and you get Perennial, Shrub and Subshrub, native in AK, CAN and
L48. Query Himalayan blackberry (symbol RUAR9) and the same fields come
back reading Introduced. It also carries noxious status, invasive status
and legal status fields where an agency has designated one, and it has a
state search that will build you a plant list for a state.

**The Fire Effects Information System, for depth.** USDA Forest Service,
`https://research.fs.usda.gov/feis`. FEIS describes itself as a
collection of scientific literature syntheses about fire effects on
individual species and ecosystems, and its species reviews cover life
history, ecology and relationship to fire, each with its own literature
citations. Do not be put off by the fire framing. These are the deepest
free species accounts that exist, they cover habitat, phenology, wildlife
use and human use, and they carry their sources. Most of the ecological
substance in the Silverdale plant records traces to FEIS.

**US Fish and Wildlife Service, for what is federally listed.**
`https://www.fws.gov/`. For "what listed species occur at this specific
location", the tool is IPaC (`https://ipac.ecosphere.fws.gov/`), a
project planning tool that cross-references a location you draw against
species range and listing data. It is built for environmental review, and
FWS says it is for private citizens as well as public employees, so there
is nothing improper about a resident using it to find out what is listed
under their own feet.

**NOAA Fisheries, for marine and anadromous species.** Salmon, steelhead,
marine mammals and marine fish listings live with NOAA rather than FWS,
which is why the Puget Sound Chinook listing above is a NOAA document.
If your question is about something that swims in salt water, start
there.

**iNaturalist, which is a starting point and never the authority.** It is
genuinely useful for narrowing an unknown down to a plausible genus, and
its observation maps will show you what people near you are actually
finding. Understand what its quality label means before you lean on it.
An observation reaches "Research Grade" when more than two thirds of the
identifiers who weighed in agree on a species-level identification, plus
housekeeping conditions like a date, a location, and a photo or sound.
That is a community agreement threshold. It is not an expert
determination, it is not a verification, it can change when opinion
shifts, and iNaturalist itself describes the underlying data quality
assessment as a summary of an observation's accuracy, completeness and
suitability for sharing with data partners, which is a carefully narrower
claim than "this identification is correct". Use it to generate
candidates. Never use it to close question 4.

**Where the federal sources stop.** They stop above your county. This is
worth internalising because it is the reason a local list has to exist at
all. The shipped Silverdale file says so repeatedly and in plain terms:
for common camas, no public-domain source consulted confirms Kitsap
County specifically, so the record calls its presence plausible and
unconfirmed. For meadow death camas, the Forest Service documents it on
Puget Sound prairie in the next county over, and the record still says
county-level presence here is likely but unconfirmed, while explicitly
warning against using an elevation range from Intermountain floras to
rule it out at sea level. That is what honest federal-source-only
coverage looks like at county scale. Below that line, you are into state
and county bodies, which is the last section of this guide.

## Lookalikes are the entire subject

Nobody eats water hemlock on purpose.

That sentence is the whole safety argument. USDA calls western water
hemlock the most violently toxic plant that grows in North America, and
says only a small amount of the toxic substance is needed to produce
poisoning in humans. Nobody looks at it, understands what it is, and eats
it anyway. People die from it because they believed it was something
else.

And look at what they believed it was. The most dangerous confusion in
wet ground on this peninsula is with *Sium suave*, which is native here,
which foraging literature treats as an edible root, which grows in the
same marshes and stream edges with the same white flowers in the same
umbrella-shaped clusters, and whose USDA common name is **hemlock
waterparsnip**. The word "hemlock" is in the name of the edible one. A
person who has learned "avoid hemlock" has learned something that will
not save them, because the name is on both.

There are real distinguishing characters. USDA states that on water
hemlock the side veins of the leaves lead to notches rather than to tips
at the outer margin, and that its thick rootstalk holds small chambers of
a brown or straw-coloured liquid released when the stem is cut. Both are
genuine. Both are also useless as a beginner's safety procedure: the vein
character needs a single leaflet held to good light and an eye that has
done it before, and the root character requires you to have already cut
into a plant that poisons people by being cut.

So the shipped record does the thing a book almost never does. It says
the honest thing: a beginner cannot reliably tell these two apart in the
field, experienced foragers have died getting it wrong, and the working
rule is to eat no carrot-family plant gathered from wet ground at all.

**That sentence is the most valuable thing on the page.** When a species
record tells you a distinction is beyond you, believe it, and notice that
it took more integrity to write than a list of field marks would have.

Four patterns are worth learning from how the lookalike entries in this
dataset are built, because they generalise to anywhere.

**The catch-all entry is deliberate.** Water hemlock's record does not
only name specific twins. It carries an entry for wild carrot-family
plants generally, and the record says outright that this catch-all is the
part that actually protects people, because it never designates any
white-umbel plant as safe. Poison hemlock's record does the same, and is
even blunter: before it bolts, poison hemlock is a low rosette of finely
divided leaves with no purple-blotched stem to check because there is no
stem yet, and at that stage no field character offered will close the
gap, because there is not one that works.

**The route matters, not just the resemblance.** Water hemlock's lookalike
list includes broadleaf cattail, which looks nothing like it. It is on the
list because cattail rhizomes are dug from the same marsh mud in autumn
and winter, when the standing plants are gone and nothing above ground
says which plant a root came from. That is not a visual confusion. It is
the actual path by which a person meets the poison. Ask not only "what
looks like this" but "what would I be doing when I met this".

**A negative test only works in one direction.** Death camas has grass-like
leaves and a bulb, and is confused with wild onion. The check is smell:
onions smell of onion and death camas does not. The shipped record insists
this test be used one way only. No onion smell means do not eat it, full
stop. Smelling onion does not prove a particular bulb is an onion, because
a bulb picks up the smell from your hands, your knife, or a crushed onion
lying next to it in the basket. So smell every bulb separately, with clean
hands, before it joins the others. The same asymmetry applies to poison
hemlock's purple-spotted stem: the spots are a real mark, and their
absence proves nothing, because they develop as the plant bolts and a
first-year rosette has no marked stem at all.

**Sometimes the identification and the harvest happen months apart.** This
is the most instructive idea in the whole dataset. Common camas and meadow
death camas share wet spring meadows, both are dug for bulbs, and USDA
states that death camas has been mistaken for camas, wild onion and sego
lily especially when flowers are lacking. Out of bloom there is no
reliable field character at all. USDA's number for the consequence: one or
two death camas bulbs is enough to cause severe illness in a child, four
or five can cause death.

The answer is not a better field mark. It is a calendar. Walk the meadow
while it is in bloom, when camas shows blue to bluish-violet flowers and
death camas shows creamy white ones, mark the individual plants you want,
and come back later and dig only the plants you marked, one at a time. The
flower does the identifying, and the marker carries that identification
forward to digging day. If you arrive at a meadow with no flowers and no
marks, you have no way to do this safely, and the correct action is to
leave and come back in May.

Generalise that. The identification does not have to happen on the day you
want to harvest, and for the dangerous cases it must not. The knotweed
record reaches the same conclusion from the other end: identify the stand
in full summer growth, mark it, and return to that marked stand in spring,
rather than deciding what a young shoot is on the day you want to eat it.

## How to read a species record

Here is what each field in `data/locales/silverdale_wa/species.json` is
for, and more importantly what each one does not say. The shape is defined
in `schemas/locale.toml` and is deliberately location-generic: nothing
about the schema knows it is describing Washington.

**`common_name` and `scientific_name`.** The common name is what people
here call it, which is exactly the thing you just learned not to trust.
The binomial with its authority is the checkable claim. When a record
carries a naming note, read it, because that is usually where an authority
disagreement or a retired name is flagged.

**`kind`, `form`, `status`.** `status` is native, introduced, invasive or
cultivated, and it is a statement about *here*. The Silverdale file is 102
native, 9 introduced and 14 invasive out of 125. "Invasive" is not a
description of the organism's character; it is a designation some agency
has made about it in this place.

**`protected`.** The legal rule, and what rule it is. Note that this field
does two jobs. It carries conservation protection (bull trout is ESA
threatened and closed to fishing; the Olympia oyster is a state candidate
species with wild harvest closed), and it also carries harvest rules that
are nothing to do with rarity. Salal and cascara are neither rare nor
protected, and both have a permit threshold under Washington's specialized
forest products law: more than twenty pounds of cut evergreen foliage, or
more than five pounds of cascara bark, needs a permit before you harvest
or transport it. Picking a handful of berries is unaffected; commercial
brush picking, a real industry on this peninsula, is not. A `null` here
means no protection was found, which is not the same as "verified
unprotected".

**`edible` and `edible_detail`.** `edible` is one of three words. In
Silverdale the split is 11 yes, 62 conditional, 52 no. Note how small the
unconditional yes column is, and note that the largest column by far is
conditional. **"Conditional" means the condition is stated, and every
condition binds.** Manila clam is conditional on three separate
authorities agreeing on the day. Black-tailed deer is conditional on a
licence, a tag, an open season, the right game management unit, the bag
limit, and current disease testing rules, and then separately on the local
ordinance about discharging a firearm on that particular parcel, because
where you see a deer and where you may lawfully take one are often not the
same place. A conditional entry with one condition met is not one third
safe.

**`toxic`. And here is the field people misread.** A `null` or an absent
toxicity note **is not a safety clearance**. It means no toxicity was
documented in the sources consulted. Fifty-seven of the 125 Silverdale
records have no toxicity content, and the reasons vary enormously. Three
worked examples of the same blank meaning three different things:

- **Western hemlock** has no eating hazard recorded, and the record says
  why that is weak evidence: the sources describing it are forest ecology
  and forest products reviews, so a clean toxicity record means *not
  reported* rather than *tested and cleared*.
- **Salal** likewise, and its record makes the same point with a further
  twist: the fruit is documented as eaten raw, but the same Forest Service
  review notes that many *Gaultheria* species contain oil of wintergreen,
  which is methyl salicylate, so a cleared berry is not a cleared leaf tea.
  A clearance covers a part, a preparation and a quantity, not a species.
- **Japanese knotweed** is recorded by Washington's noxious weed board as
  not known to be toxic, and its record immediately says what that is: a
  weed-control agency's statement about contact and livestock hazard, not
  a food-safety clearance, and not to be read as one. The hazard that
  actually applies to eating knotweed is herbicide residue from the
  mandated control programme, which is not a property of the plant at all.

And the sharpest case: **hemlock waterparsnip has no documented toxicity
and a lethal lookalike.** Its `toxic` field is empty. Its `edible_detail`
says do not eat this plant. Reading the toxicity field alone would get
somebody killed. The record states the provenance asymmetry explicitly: no
public-domain federal source opened for it documents the plant as a human
food, and none documents it as toxic either, so the edibility claim is
unverified while the hazard beside it is verified in detail, and *when
only half a pair is sourced, the sourced half is the one to act on.*

**`lookalikes`.** Each entry has a name, a link to another record where one
exists, a danger rating, and a paragraph on telling them apart. Read these
first, not last. A `null` id means the species is named but not yet in the
dataset, which is a gap to fill rather than a reason to ignore the
warning. And watch for the two honest non-answers, which are the most
valuable entries of all: "a beginner cannot reliably tell these apart",
and "no toxic lookalike found", which the blackberry and knotweed records
both spell out as **recorded as none found rather than none exists.**

**`season` and `habitat`.** Where and when, including the traps. Water
hemlock's season note is a warning in itself: the tuberous root is the
most poisonous part, and it is in the ground all year including winter,
when nothing above ground shows where the plant is.

**`uses` and `sources`.** Sources are per record, with URLs, so every claim
can be walked back to the body that made it. That is what makes the file a
reference rather than a rumour, and it is the standard to hold any species
list to, including one you build yourself.

The licence line at the top of the file records where the whole thing came
from: US federal sources, which are public domain, plus Washington State
agencies, whose facts are restated in our own words with the agency cited
because state works are not covered by the federal public-domain rule.
Nothing came from GBIF, iNaturalist, Wikipedia or OpenStreetMap. If you
build a list of your own and intend to share it, keep the same discipline,
because the licence travels with every copy.

## Protection, and why it keeps surprising people

Most people's mental model is that rare and dramatic things are protected
and ordinary things are not. That model is wrong in both directions, and
the errors are asymmetric: mistaking a protected thing for an unprotected
one is an offence, and mistaking an unprotected thing for a protected one
costs you nothing.

**Nearly every native bird is protected, by default.** The federal
Migratory Bird Treaty Act prohibits the take, which FWS spells out as
including killing, capturing, selling, trading and transport, of protected
migratory bird species without prior authorization. The list of covered
species is published in the Code of Federal Regulations at 50 CFR 10.13,
and it is long. On top of that, Washington's protected wildlife rule
sweeps up everything left over: all birds not classified as game birds,
predatory birds, endangered, threatened or sensitive species are
protected, and Washington's predatory birds are only six named species
(black-billed magpie, American crow, European starling, house sparrow,
rock dove and Eurasian collared dove). The practical consequence is that a
Steller's jay raiding your campsite is protected wildlife with no season,
no licence and no bag limit, which means no lawful way to take one. Being
a nuisance is not a legal exception. The answer to a jay in your food is a
better container.

**A thing can look utterly ordinary and be named in the rule.** The
Douglas squirrel again. It is the small loud reddish squirrel scolding you
from a fir in every conifer stand on the peninsula, and it is named
explicitly in WAC 220-200-100. Its own record notes that an earlier draft
said only that it was not a game animal and left the status unverified,
which understated the protection. If you take a Douglas squirrel believing
it was an eastern gray, you have taken protected wildlife, and the
lookalike entry exists for precisely that reason.

**Federal listing can attach to a fish you would otherwise keep.** Bull
trout is ESA threatened, and Washington's rules close it outright. WDFW is
explicit about what to do if you hook one by accident: it must not be
taken out of the water, and must be de-hooked and released immediately.
Not lifted for a photograph, not weighed, not netted onto the bank.
Steelhead is likewise threatened here, and wild ones must always be
released. Both are trout among other trout, in water where fishing for
other species is perfectly legal.

**And protection is not the only legal gate.** The salal and cascara
permit thresholds are not conservation rules. The requirement that you buy
a Puget Sound crab endorsement and carry a Dungeness catch record card
before crabbing in Puget Sound applies even when the crab you are actually
keeping is a red rock crab, which needs no card of its own. None of that
is findable by looking at the animal.

The general rule this adds up to: **assume protected until you have
checked, not the other way round.** Checking costs a few minutes. The
other error costs a citation, and in the case of a federally listed
species, considerably more.

## Building your own list, wherever you live

The Silverdale file is one place. Here is how to assemble the equivalent
for yours, in the order that wastes the least time. This is the
transferable skill, and it is the real point of this guide.

**1. Your state fish and wildlife agency, first.** Every state has one and
it is the single highest-yield source, because it holds three separate
things you need: the classification lists (what is a game animal, what is
a furbearer, what is protected, what is unclassified), the current
seasons and limits, and species pages for the animals themselves. It is
also the body whose rules change most often, sometimes by emergency
rule mid-season, so it is the one to check before a trip rather than once
a year.

**2. Your state noxious weed board.** Usually a state board with county
coordinators under it. This answers a question no field guide will: which
plants are designated, which class they are in, whether control is legally
required, and therefore which plants are likely to have been sprayed.
Washington's boards are at `https://www.nwcb.wa.gov/` and list the county
contacts. Your county coordinator is a real person who will answer the
phone about a specific stand.

**3. USDA PLANTS, filtered down to your state.** `https://plants.usda.gov/`.
Use the state search to build a list, and the advanced search filters,
which include nativity status, invasive and noxious status, rarity status,
duration, growth habit and state or province. This gives you the
name-and-status backbone for plants, in public domain, at state
granularity. Expect it to stop above your county; that is what the next
two entries are for.

**4. FWS IPaC for what is federally listed at your location**, and NOAA
Fisheries if you are anywhere near salt water or anadromous fish. Do this
before you think you need it. The surprise in the Silverdale data was not
the whale, it was the trout.

**5. Your state health department, for shellfish, and only live.** If you
are coastal, find the marine biotoxin programme. In Washington that is a
Shellfish Safety Map plus a recorded closure hotline on 1-800-562-5632.
This is the one authority you cannot read once and file, because closures
have no season. Check it the day you go, every time.

**6. Your cooperative extension service, for everything local and human.**
Every state's land-grant university runs one, created by the Smith-Lever
Act of 1914, and USDA NIFA says the land-grant system reaches every county
in the United States. The directory of extension partners is at
`https://nifa.usda.gov/land-grant-colleges-and-universities-partner-website-directory`.
Extension is where you find the county-level answer, the master gardener
who can look at your sample, and advice written for your actual soil and
season. Extension publications are usually copyrighted, so cite them and
write the fact in your own words rather than copying.

**7. Then write it down in the same shape.** Name, binomial, status here,
protection here with the rule cited, edibility with its conditions stated,
toxicity with the part and the effect, lookalikes with how to tell them
apart, season, habitat, and a source URL per claim. The shape is in
`schemas/locale.toml`. A list without the lookalike column is not a safety
document, it is a temptation.

## The year is part of the identification

Phenology is the study of when things happen: leaf-out, bloom, fruit,
salmon runs, arrivals and departures. It belongs in this guide rather than
in a separate one because for several of the cases above, timing *is* the
identification. Camas is identifiable in May and anonymous in September.
Knotweed is identifiable in August and ambiguous in April. Water hemlock's
root is in the ground all winter with nothing above it to mark the spot.
A species list without a calendar beside it is missing the dimension that
makes half of it usable.

The month-by-month calendar for this place is in
[Where You Are: Silverdale, Washington](../locale/silverdale_wa.md), and
it labels each event with how confident it is, distinguishing a date
computed from observations from a regional generalisation. Note also what
that calendar deliberately refuses to contain, because the refusals teach
as much as the entries: no monthly biotoxin season, because closures have
no season and a calendar must not imply the tide answers that question; no
mushroom dates, because fruiting here is triggered by rain rather than by
the calendar; and no fixed shellfish beach seasons, because those are set
beach by beach and annually, and reprinting last year's would be worse
than silence.

## What this guide does not qualify you to do

It does not qualify you to eat anything.

That is not modesty. Everything above is about how to *learn* the living
things around you: how to name them checkably, how to find out what
protects them, how to read a record honestly, and how to ask "what else
could this be" before "does this match". Those are the foundations. They
are not the skill.

Foraging is taught separately, in its own guides, and the separation is
deliberate. Identifying wild plants to species, knowing what is reliably
edible where you live, recognising the poisonous ones, mushrooms (which
are a higher-stakes discipline than plants and get their own treatment for
that reason), seasonality, harvesting without ruining the resource, and
shoreline and shellfish harvest are each their own topic with its own
hazards. Nothing in this document is a substitute for any of them, and
nothing in it should be used as a licence to put something in your mouth.

Three things to carry out of here instead:

- **The local answer is the only answer.** A regional guide cannot hold
  your county's legal status, your beach's closure status, or today's date.
- **The fourth question is the one that hurts.** Not "does this match" but
  "what else could this be", asked until you can rule each one out or
  admit you cannot.
- **A blank field is a gap in the sources, not a clearance.** No documented
  toxicity means nobody wrote it down. It does not mean somebody checked.

When you are ready for the foraging topics, go to them from the beginning
and let them teach it properly. A person who has read this guide and stops
here has lost nothing. A person who has read this guide and treats it as
permission has misread it entirely.

## Sources

Grouped by what kind of authority each one is. Government publications are
listed first because they are public domain and can be redistributed with
this guide; the rest cannot.

### United States government (public domain)

- Integrated Taxonomic Information System (USGS and Smithsonian
  partnership). Site overview and mission; the taxonomic web services used
  for the worked examples in this guide. https://www.itis.gov/
- ITIS. `searchByCommonName` for "hemlock", returning TSN 183396
  (*Tsuga*), TSN 183421 (*Pseudotsuga macrocarpa*), TSN 29473 (*Conium
  maculatum*) and TSN 29456 (*Cicuta maculata*), queried for this guide.
  https://www.itis.gov/ITISWebService/jsonservice/searchByCommonName?srchKey=hemlock
- ITIS. `getFullRecordFromTSN` for TSN 20889, *Polygonum cuspidatum*
  Siebold & Zucc., returning taxon usage rating "not accepted",
  unaccept reason "synonym", accepted name *Fallopia japonica* var.
  *japonica* (TSN 823876), and the common names Japanese knotweed,
  fleeceflower and Mexican bamboo.
  https://www.itis.gov/ITISWebService/jsonservice/getFullRecordFromTSN?tsn=20889
- ITIS. `searchByScientificName` for *Tsuga heterophylla*, returning TSN
  183400 and the authority (Raf.) Sarg.
  https://www.itis.gov/ITISWebService/jsonservice/searchByScientificName?srchKey=Tsuga%20heterophylla
- USDA Natural Resources Conservation Service. PLANTS Database. Plant
  profile fields (accepted name and symbol, duration, growth habit,
  native status by region, synonyms, noxious and invasive status),
  verified against the profiles for *Gaultheria shallon* (GASH) and
  *Rubus armeniacus* (RUAR9). https://plants.usda.gov/
- USDA PLANTS. State Search, for building a plant list for a state.
  https://plants.sc.egov.usda.gov/state-search
- USDA PLANTS. About Advanced Search (the filters: duration, group,
  growth habit, invasive and noxious status, nativity status, rarity
  status, state or province, wetland region). The site is a
  single-page application whose text could not be fetched directly when
  this guide was written, so the filter list is taken from the USDA help
  page's own indexed description rather than read off the page.
  https://plants.sc.egov.usda.gov/about_adv_search.html
- USDA Forest Service, Rocky Mountain Research Station. Fire Effects
  Information System: what it is and what a species review contains.
  https://research.fs.usda.gov/feis
- US Fish and Wildlife Service. Migratory Bird Treaty Act of 1918 (the
  prohibition on take including killing, capturing, selling, trading and
  transport without prior authorization; the species list published at
  50 CFR 10.13). https://www.fws.gov/law/migratory-bird-treaty-act-1918
- US Fish and Wildlife Service. Information for Planning and Consultation
  (IPaC), for identifying federally listed species at a specific
  location. https://ipac.ecosphere.fws.gov/
- USDA National Institute of Food and Agriculture. Land-grant University
  Website Directory (Cooperative Extension, established by the Smith-Lever
  Act of 1914, reaching every county in the United States).
  https://nifa.usda.gov/land-grant-colleges-and-universities-partner-website-directory
- NOAA Fisheries. Puget Sound Chinook salmon ESA listing and the extent
  of the evolutionarily significant unit; coho ESA listings, none of which
  cover Puget Sound. Cited through the species data file, which records
  the NOAA URLs per record.
  https://www.fisheries.noaa.gov/west-coast/endangered-species-conservation/puget-sound-chinook-salmon
- USDA Agricultural Research Service, Poisonous Plant Research Laboratory.
  Water hemlock (*Cicuta douglasii*) and poison hemlock (*Conium
  maculatum*): toxicity, the leaf vein character, the chambered rootstalk,
  and the documented confusions with parsley, parsnip and anise. Cited
  through the species data file.
  https://www.ars.usda.gov/pacific-west-area/logan-ut/poisonous-plant-research/docs/water-hemlock-cicuta-douglasii/

### State agencies (cited as the authority; text written here)

State agencies are not covered by the federal public-domain rule, so these
are cited for the fact and the sentences are ours.

- Washington State Department of Health, Marine Biotoxins. What biotoxins
  are, the Shellfish Safety Map, the recorded closure hotline on
  1-800-562-5632, and that cooking and freezing do not destroy the toxins
  because they are part of the shellfish meat.
  https://doh.wa.gov/community-and-environment/shellfish/recreational-shellfish/illnesses/biotoxins
- Washington Department of Fish and Wildlife. Species classification,
  seasons, limits, licences and endorsements; bull trout, Olympia oyster,
  Douglas squirrel and shellfish beach status. Cited per record in the
  species data file. https://wdfw.wa.gov/
- Washington State Noxious Weed Control Board. Class designations and the
  county coordinator contacts. https://www.nwcb.wa.gov/
- Washington Administrative Code 220-200-100 (wildlife classified as
  protected, and the catch-all covering all birds not otherwise
  classified) and WAC 220-400-020 (game animals and furbearers), and RCW
  76.48.021 and 76.48.031 (specialized forest products permits for salal
  foliage and cascara bark). Cited through the species data file.

### Third-party services, described rather than relied upon

- iNaturalist Help. What the Data Quality Assessment is and how an
  observation becomes Research Grade (more than two thirds of identifiers
  agreeing at species level, plus date, location and media), and the
  narrower claim the assessment actually makes.
  https://help.inaturalist.org/en/support/solutions/articles/151000169936-what-is-the-data-quality-assessment-and-how-do-observations-qualify-to-become-research-grade-

### The locale data this guide's worked examples rest on

Every Silverdale species example above (legal status, edibility
conditions, toxicity, lookalikes, seasons and habitat) comes from
`data/locales/silverdale_wa/species.json`, which records its own source
URLs per record and its own licence at the top of the file. Its
provenance is US federal sources plus Washington State agencies restated
in our own words, with nothing taken from GBIF, iNaturalist, Wikipedia or
OpenStreetMap. The record shape is defined in `schemas/locale.toml`. The
generated reading copy of the place, including the month-by-month
calendar, is
[Where You Are: Silverdale, Washington](../locale/silverdale_wa.md).

### Companion guides in this collection

- [What Soil Is](what_soil_is.md)
- [Your Soil, Specifically](your_soil_specifically.md)
- [The Growing Calendar](the_growing_calendar.md)
- [Reading the Tide](reading_the_tide.md)
- [The Hazards Your Place Actually Has](the_hazards_where_you_live.md)
- [Where You Are: Silverdale, Washington](../locale/silverdale_wa.md)
