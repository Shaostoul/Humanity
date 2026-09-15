# Identifying Wild Plants

Read this part before anything else.

**This document cannot teach you to identify a plant, and nothing in it
should be used as permission to eat one.** It teaches the method and the
discipline: what identification actually is, what it is not, which parts
of a plant carry the answer, and how to know when you do not have one.
It contains no key, no species list, and no field marks you could act on
tonight.

That is deliberate, and it is not caution for its own sake. The failure
mode in this subject is death, not disappointment. USDA's own words for
the worst plant in this guide's home region: water hemlock is the most
violently toxic plant that grows in North America, and only a small
amount of its toxin is needed to produce poisoning in humans. Nobody eats
it on purpose. People die from it because they believed it was something
else.

The way people actually learn this is in person, from somebody who
already knows, one species at a time, over years. A book, a website and a
phone app can all help that process along. None of them replaces it.
If you read this whole document carefully and stop here, you have lost
nothing.

Its companion, [The Species Where You Live](the_species_where_you_live.md),
covers the four questions to ask about any living thing, why a common
name is not an answer and a binomial is, and which authorities answer
which question. Read that one first. This document does not repeat it,
and it assumes you have it.

## The positive identification rule

This is the spine of everything below, so it goes first.

**You do not identify a plant by ruling out what it is not. You identify
it by matching every diagnostic character of the thing you believe it is,
and if one character does not match, or you cannot check it, you stop.**

The failure this rule exists to prevent has a specific shape, and it
feels like competence while it is happening. You look at a plant, you
look at a description, and you tick features off. Leaf shape, yes.
Flower colour, yes. Habitat, yes. Height, about right. Each tick makes
you more confident. That process is confirmation, not identification, and
confidence built that way is worth nothing, because **a plant with nine
matching features and one deadly twin is not nine tenths safe.** The twin
shares those nine features. That is what makes it a twin.

So identification has two halves, and both must complete:

1. **Every diagnostic character of the species you believe it is,
   checked, on this individual plant, now.** Not most of them. Not the
   ones that were easy to see. Every one. A character you could not check
   because the plant is the wrong age, or the light was bad, or it would
   have meant cutting something you should not cut, is a character that
   did not match.
2. **Every dangerous species it could otherwise be, eliminated by name.**
   Not "I have never seen that here". Eliminated, one at a time, by a
   character you actually checked.

If either half is incomplete, you have not identified the plant. You have
a guess. Guesses are fine. Eating them is not.

The phrase to be afraid of is **"it looks like"**. Not because looking is
useless, but because looking is where the whole error lives. FDA, writing
about mushrooms but describing the same human being, records that many
cases of poisoning have happened in people who were using field guides,
had a lot of experience, and were sure they had picked the right kind.
The same page notes that poisonings are almost always caused by wild
specimens collected by nonspecialists, and then adds, in parentheses,
that specialists also have been poisoned.

## The fourth question, worked three times

The companion guide gives four questions, and says the fourth one is the
one that hurts: not "does this match" but **"what else could this be"**.
Here is what that question looks like when you actually run it, using
three real pairs from the species file that ships with this project,
`data/locales/silverdale_wa/species.json`.

Read these as demonstrations of a method. They are not instructions for
telling these plants apart, and each one ends by saying so.

### Wet ground: water parsnip and water parsley against water hemlock

Three plants in the same marsh. All three are in the carrot family. All
three have divided leaves and small white flowers in umbrella-shaped
clusters. Two of them, hemlock waterparsnip (*Sium suave*) and water
parsley (*Oenanthe sarmentosa*), are native here and are treated as food
in foraging literature. The third, western water hemlock (*Cicuta
douglasii*), is the one USDA calls the most violently toxic plant on the
continent.

Notice the name first. USDA's own common name for the edible one is
**hemlock** waterparsnip. A person who has carefully learned "avoid
hemlock" has learned something that will not save them, because the word
is on both plants.

There are two real characters, and both come from USDA. On water hemlock,
the side veins of the leaves lead to notches rather than to tips at the
outer margin. And the thick rootstalk contains small chambers holding a
highly poisonous brown or straw-coloured liquid, released when the stem is
broken or cut.

Now look at what those characters cost you. The vein character needs a
single leaflet held up to good light and an eye that has done it before.
The root character requires you to have already cut into a plant that
poisons people by being cut; USDA is explicit that the liquid is released
when the stem is broken, so cutting, mowing, pulling or clearing it is
itself an exposure.

The shipped record does the thing a book almost never does, and says the
honest thing outright: **a beginner cannot reliably tell these two apart
in the field, and experienced foragers have died getting it wrong.** Its
working rule is to eat no carrot-family plant gathered from wet ground at
all.

One more thing from that record, because it is the part that generalises
furthest. Water hemlock's lookalike list includes broadleaf cattail,
which looks nothing like it. Cattail is on the list because cattail
rhizomes are dug from the same marsh mud in autumn and winter, when the
standing plants are gone and nothing above ground says which plant a root
came from. That is not a visual confusion. It is the actual route by
which a person meets the poison. So the question is not only "what looks
like this", it is **"what would I be doing when I met this"**.

### Roadside: wild carrot against poison hemlock

Poison hemlock (*Conium maculatum*) is deadly in every part. USDA states
that all parts, leaves, stem, fruit and root, are poisonous. Wild carrot
(*Daucus carota*), Queen Anne's lace, is the ancestor of the garden
carrot and grows on the same road verges and field edges.

Here there is a genuinely useful check, and this time it is a state
authority rather than a federal one, so the fact is Washington's and the
sentence is ours. Washington's noxious weed board describes poison hemlock
as a very tall biennial, up to about 12 feet, with hollow, hairless stems
carrying noticeable purple blotches; it describes wild carrot as smaller,
covered all over with coarse stiff hairs, and without purple blotches on
the stems. So a hairy stem points at wild carrot, and a hairless stem
with purple blotches points at poison hemlock.

Then run the test backwards and watch it fail. **Finding hairs is worth
something. Not finding purple spots is worth nothing.** The spots develop
as poison hemlock bolts, and a first-year poison hemlock is a low rosette
with no stem to check at all. That rosette stage runs through autumn,
winter and early spring, which is exactly the season you would be digging
a first-year carrot root.

This asymmetry is general and it is worth carrying out of this document
as a habit. A field character usually works in one direction only. It can
condemn, or it can support, rarely both. Before you lean on one, ask
which direction it runs in, and what its absence actually proves.

The same pattern appears in the shipped record for nodding onion. Onions
smell of onion and death camas does not, so **no onion smell means do not
eat it, full stop**. But smelling onion does not prove that the bulb in
your hand is an onion, because a bulb picks up the smell from your
fingers, your knife, or a crushed onion lying beside it in the basket.
Hence the procedure: smell every bulb separately, with clean hands,
before it joins the others. Note also that the shipped record labels this
onion-smell test honestly, as something no public-domain federal source
states in so many words. The half of it that is sourced is the dangerous
half: USDA records that death camas is mistaken for wild onion.

And the honest end of this pair, as the records give it: at the rosette
stage a beginner cannot separate wild carrot from poison hemlock, and
people better qualified than a beginner have died making exactly that
call. There is no field character on offer here that closes that gap,
because there is not one that works.

### Meadow: common camas against meadow death camas

This is the most instructive pair in the whole subject, because the
lesson it teaches is not about looking at all.

Common camas (*Camassia quamash*) and meadow death camas (*Zigadenus
venenosus*) share wet spring meadows, both are grass-leaved, both grow
from a bulb, and both are dug for that bulb. USDA's plant guide for death
camas states that the common name refers to the toxicity of the plant and
its similarity in appearance to camas, and that it has been mistaken for
other edible bulbous plants such as wild onion, sego lily and camas,
**especially when flowers are lacking**. The same guide gives the number
that matters: eating one or two death camas bulbs is enough to cause
severe illness in children, and four or five can cause death.

In bloom, there is a check anyone can make from standing height. The
Forest Service describes common camas flowers as blue to bluish-violet,
with tepals 12 to 35 millimetres long. USDA describes death camas petals
as creamy white, about a quarter inch long, with a large gland at the
base. Blue against creamy white, at arm's length. That is as easy as
identification in this subject ever gets.

Out of bloom there is no reliable field character at all, and the bulbs
cannot be told apart safely by a beginner.

So the answer is not a better field mark. **The answer is a calendar.**
The identification happens in May, when the meadow is in flower. The
harvest happens months later. Those are two different events, and the
only thing that can carry the identification from the first to the second
is a physical marker left on the individual plant, not your memory of
where you walked. Walk the meadow in bloom, mark the individual plants
you want, come back later, and dig only the plants you marked, one at a
time, never working through a patch.

If you arrive at a meadow with no flowers and no marks, you have no way
to do this safely, and the correct action is to leave and come back in
May.

## The parts you must look at, and the parts people look at instead

Beginners identify plants by their leaves. Leaves are what is there for
most of the year, they are what a photograph captures, and they are what
a phone app is looking at. That is a rule of thumb rather than a
measured finding, but the consequence of it is visible in every pair
above, and the consequence is sourced.

Look at where the answer actually sat in each case:

- Water hemlock against water parsnip: leaf **venation** at a fine scale,
  and the **rootstalk**, underground.
- Poison hemlock against wild carrot: the **stem**, hairs and blotches.
  Both plants have fern-like, finely divided leaves. USDA describes
  poison hemlock's leaves as delicate, like parsley, which is to say the
  leaves are the source of the confusion rather than the solution to it.
- Camas against death camas: the **flower**, colour and size. Both have
  grass-like leaves.

Three lethal pairs, three separators, and not one of them is a leaf
outline. The answer sat in the flower, the fruit, the root, the stem
cross-section and the smell. Those are the parts that carry the
identification, and it is not an accident: they are the parts that vary
least within a species and most between species.

There is a fourth pair worth adding here, because it makes the same point
about the part people actually reach for. Red baneberry (*Actaea rubra*)
is a knee-high woodland herb whose berries the Forest Service describes
as showy, poisonous, red or occasionally white, with a poisonous
essential oil or glycoside found in all parts but most concentrated in
the berries and the root, and the National Park Service states that the
berries are poisonous and will often send the heart into cardiac arrest.
It grows in the same moist shaded forest as claspleaf twistedstalk, whose
red berries are eaten; the Forest Service describes twistedstalk as
carrying only one flower on each flower stalk, hanging downward, giving
rise to a single elliptic yellow or red berry which is edible.

**The colour of the berry tells you nothing**, since baneberry's own
fruit is red or white and both are poisonous. What separates the two is
the architecture of the plant holding them: twistedstalk hangs one berry
per stalk, dangling under an arching stem, while baneberry bunches its
berries on one upright stalk held above the leaves. So the answer is not
in the fruit you want to eat. It is in the plant the fruit is attached
to, which is the part a person picking berries never looks at.

And even that pair ends where all the others do. The Forest Service,
writing about twistedstalk itself, warns that it can be mistaken for the
poisonous *Veratrum* and says that perhaps it is better left unsampled.

Two consequences follow, and the second is the dangerous one.

**First, a description you cannot check is a character that did not
match.** If a key asks about the stem and there is no stem yet, you are
not most of the way there. You are stopped.

**Second, the parts that carry the answer are the parts that are absent
for most of the year.** A flower is present for a few weeks. A fruit for
a few more. A first-year rosette has no stem. A root requires digging,
and in one of the three cases above, cutting the root is itself the
exposure. So the reliable characters and the convenient characters are
almost exactly opposite sets, and a person working from leaves in
September is working from the weakest evidence at the worst time.

That mismatch is the whole reason the next section exists.

## The year is part of the identification

Identification and harvest are two separate events, and for the dangerous
cases they happen months apart.

The camas pair is the clean demonstration, but the pattern repeats.
Poison hemlock's purple-blotched stem exists only after it bolts; its
rosette sits there all winter with nothing to check. Water hemlock's
tuberous root, which USDA identifies as where the toxin is principally
found, is in the ground year round, including the months when nothing
above ground marks the spot. Japanese knotweed, in the shipped records,
gets the same treatment from the other end: identify the stand in full
summer growth, mark it, and return to that marked stand in spring, rather
than deciding what a young shoot is on the day you want to eat it.

The general form: **identify the individual plant when it is showing you
the most, mark it physically, and let the marker carry the identification
forward to the day you harvest.** Your memory of a location is not a
marker. A photograph with a date is evidence about a plant, not about the
plant now in your hand.

This is also why a species list without a calendar beside it is missing
the dimension that makes half of it usable. The month-by-month calendar
for this project's worked locale is in
[Where You Are: Silverdale, Washington](../locale/silverdale_wa.md).

## The carrot family, and why you leave all of it alone

Apiaceae, the carrot or parsley family, is the one family a beginner
should treat as a single closed door.

Look at what is inside it. Water hemlock, which USDA calls the most
violently toxic plant in North America, whose toxin cicutoxin acts
directly on the central nervous system as a violent convulsant. Poison
hemlock, poisonous in every part, whose piperidine alkaloids cause
progressive paralysis, and which USDA records has killed children through
whistles made from its hollow stems. And then, in the same family:
parsnip, parsley, anise, carrot, celery, dill, fennel.

Those are not two lists that happen to be adjacent. They are the same
list. USDA's account of how people are poisoned by poison hemlock names
three confusions, and every one of them is with a food: hemlock **root**
for wild parsnips, hemlock **leaves** for parsley, hemlock **seed** for
anise. Three different organs of the plant, three different foods, three
routes into a kitchen.

To a beginner, the whole family presents the same silhouette: finely
divided leaves and small white flowers in flat or domed umbrella-shaped
clusters. That silhouette is not an identification. It is a family
resemblance covering a group that contains staple foods alongside the
plant USDA calls the most violently toxic in North America and another
that is poisonous in every one of its parts.

So the instruction here is blunt and it is the shipped data's own:
**leave the entire carrot family alone until somebody has taught you, in
person, in front of the plant.** Not until you have read more. Not until
you are confident. Until a competent person has stood next to you and
shown you.

The shipped records build this in deliberately. Water hemlock's lookalike
list does not only name specific twins; it carries a catch-all entry for
wild carrot-family plants generally, and the record says outright that the
catch-all is the part that actually protects people, because it never
designates any white-umbel plant as safe. Copy that habit into any list
you build yourself. A list that names three safe plants in a dangerous
family is more dangerous than a list that names none.

## Mushrooms are a different discipline

This section is short on purpose, because mushrooms are not a harder
version of this subject. They are a different subject with a different
failure mode and a much shorter margin, and they deserve their own
treatment rather than a paragraph in a plant document.

What you need to know before you go near them:

**Identification often cannot be done by looking at all.** The Forest
Service's field guide to macrofungi states plainly that many mushrooms
can be identified only by examining the colour of spore prints or by
examining spores and tissues under a microscope. The same guide notes
that the cup at the base of an *Amanita* is often below the soil surface,
which is why mushrooms should always be dug rather than picked, and it
repeats one line at the foot of every page: do not eat any mushroom
unless you are absolutely certain of its identity.

**Some of the deadliest species cannot be separated from each other even
by an expert without a microscope.** That same guide records that
*Amanita virosa*, *A. verna* and *A. bisporigera* can only be
distinguished from one another by their spore characteristics, and that
collectively they cause 95 percent of fatal mushroom poisonings.

**There is no rule of thumb, and cooking does not help.** FDA states that
for individuals who are not trained experts there are generally no easily
recognizable differences between poisonous and nonpoisonous species, that
folklore notwithstanding there is no reliable rule of thumb for
distinguishing edible mushrooms from poisonous ones, and that most
mushrooms that cause human poisoning cannot be made nontoxic by cooking,
canning, freezing or any other means of processing. FDA records
poisonings from mushrooms that were raw, stir-fried, home-canned,
blanched and frozen, and cooked in tomato sauce, which can render the
sauce itself toxic even when no mushroom is eaten.

**The margin is the real difference.** Amatoxins are the toxin family in
the death cap and its relatives, and the shipped record for the death cap
identifies that species as responsible for most deaths following
ingestion of foraged mushrooms worldwide. FDA describes their latent
period as ranging from 6 to 48 hours, averaging 6 to 15, **during which
the patient shows no symptoms at all**. CDC's account of a
2016 California cluster describes three phases: delayed gastroenteritis
6 to 24 hours after eating, then symptomatic recovery at 24 to 36 hours
when the patient feels better, then liver and multiorgan failure
typically 3 to 5 days in. CDC notes that patients evaluated early may be
discharged home only to return later in liver failure. Of the 14 cases in
that cluster, three needed liver transplants, and one child was left with
permanent neurologic impairment.

The National Park Service at Mount Rainier states that identifying wild
mushrooms can be extremely difficult and consuming them can be deadly,
that its own species descriptions are intentionally brief and are not
sufficient to positively identify edible mushrooms, and that you should
not collect or eat any mushroom unless you are 100 percent confident of
its identification. A federal agency writing its own species
descriptions, telling you its own descriptions are not enough, is the
clearest possible statement of what reading can and cannot do here.

If you want to see what real records for this look like, the shipped
species file carries four fungi worth reading and none worth acting on:
`death_cap`, `funeral_bell`, `fly_agaric` and, on the other side,
`pacific_golden_chanterelle`, whose record spends most of its length on
the ways a chanterelle is got wrong. Read them as a picture of the
discipline. Do not read them as a key.

## The universal edibility test does not work

Survival literature carries a procedure, usually called the universal
edibility test, for deciding whether an unknown plant is safe. In its
common form it runs roughly like this: fast for some hours; separate the
plant into its parts and test one part at a time; hold a piece against
your skin, then your lip, then your tongue, waiting between each; chew a
small amount and hold it without swallowing; then swallow a small amount
and wait several hours before eating more.

**A sourcing note, stated plainly.** That description is of the procedure
as it commonly circulates. This document could not open a primary
government copy of it with readable text, so it is not quoted from one
here, and no source opened for this document endorses it as a safety
procedure. What follows is not an argument against a text. It is what the
toxicology actually says about the plants and fungi this document has
already described.

**It is defeated by delay.** The test is built on the assumption that a
dangerous plant will tell you within hours. Several of the ones that
matter most do not. FDA describes the amatoxin latent period as 6 to 48
hours with no symptoms at all. The National Park Service, writing for
visitors rather than for veterinarians, puts the effects of death camas
at 1 to 8 hours after eating the plant. Any waiting period short enough
to be practical sits inside those windows, and a waiting period long
enough to clear them is not a field procedure.

Be careful with onset times generally, including the ones in this
document's own sources. Most published poisoning timings for wild plants
come from livestock work, because that is who the research was funded to
protect. The shipped water hemlock record flags this explicitly: the
onset figures usually quoted for it are USDA's livestock figures, no
human-specific window was found in a public-domain source, and none
should be invented. A procedure that depends on knowing how long to wait
is resting on numbers that mostly are not about you.

**It is defeated by dose.** The test's later steps involve deliberately
swallowing a small amount. For the plants in this document, a small
amount is the lethal unit. USDA states that only a small amount of the
toxic substance in water hemlock is needed to produce poisoning in
humans, and that one or two death camas bulbs is enough to cause severe
illness in a child while four or five can cause death. There is no
sub-toxic tasting dose to work with.

**It is defeated by preparation.** Cooking, drying and processing do not
rescue you. FDA states that most mushrooms causing human poisoning cannot
be made nontoxic by cooking, canning, freezing or any other means of
processing, and the amatoxin literature at NIH describes the toxin as
heat stable, toxic whether eaten raw or cooked. Washington's noxious weed
board records that poison hemlock's toxins remain potent in dried plant
material, so a wilted scrap is still lethal, and the Forest Service
records that dried meadow death camas remains toxic for at least 20
years.

**It is defeated by accumulation.** Some toxins do not act on the meal
that contains them. The shipped chanterelle record, citing the Forest
Service, describes Paxillus syndrome, in which the toxins accumulate in
susceptible eaters over long periods with little effect and then the next
meal can cause sudden illness or death. Against a mechanism like that,
"I ate some and I was fine" is not evidence about anything.

**It is a folk method, and FDA names folk methods as a cause of
poisoning.** In its account of how mushroom poisonings happen, FDA
records that intoxication has occurred when people relied on folk methods
of distinguishing between poisonous and safe species.

**And it inverts the one instruction every poisoning authority gives.**
MedlinePlus, in its poisoning first aid guidance, lists among its do-not
instructions: do not wait for symptoms to develop if you suspect that
someone has been poisoned. The universal edibility test is, structurally,
an instruction to deliberately poison yourself a little and wait for
symptoms. That is the opposite procedure.

The test also fails the positive identification rule on its own terms.
It is a way of trying to prove a negative, by observing that nothing bad
happened yet. Nothing bad happening yet is not a character you checked.
It is the absence of evidence, on a clock too short to produce any.

There is one thing in the vicinity that is genuinely sound, and it
belongs to correctly identified food rather than to unknown plants: when
you eat a species for the first time, even one you have identified with
certainty, cook it properly and start with a small amount, because
individual people react badly to things that are perfectly safe for
everyone else. That is a rule about your own body, applied to a known
plant. It is not a way to find out what the plant is.

## Tools, in the order to trust them

None of these identifies a plant. You identify the plant. These are what
you identify it with, and they are listed worst-last on purpose.

| Tool | What it is actually good for | Where it stops |
|---|---|---|
| **A person who knows, standing next to the plant** | Everything. This is how the skill is transmitted, and there is no substitute for it | Finding one takes effort. The companion guide's section on cooperative extension is where to start |
| **A key in a regional flora** | Forcing you through the diagnostic characters in a fixed order, and stopping you at the one you cannot answer | Assumes flowers or fruit, and a vocabulary you have to learn. A key you fudge your way through gives a confident wrong answer |
| **A field guide** | Learning what to look at, and generating candidates | It identifies; it does not know your county, your month, or your legal status. See the companion guide |
| **A local species list with lookalikes** | The "what else could this be" half, which is the half guides leave out | Only as good as its sources, and its blanks mean nobody looked, not that nothing is there |
| **A photo identification app** | A first guess. A name to go and check properly | Measurably unreliable, including in exactly the way that hurts. See below |
| **Your own memory of a plant you saw once** | Nothing | It is not evidence about the plant in your hand |

On apps, there is a real measurement rather than an opinion. Long and
colleagues, publishing in *Clinical Toxicology* in 2023, tested four
common plant identification applications against 16 species chosen for
local availability and potential for misidentification. Overall accuracy
at genus level was 76 percent and at species level 58 percent, with a
range across apps from 94 percent down to 34 percent. The finding that
matters is not the average: **five of eleven potentially toxic species
were identified as an edible species by at least one application.** The
authors' conclusion is that the apps cannot be used to safely identify
edible plants, and that foragers need adequate botanical knowledge to
harvest wild plants safely.

Read that failure mode carefully. It is not that the apps are vague. It
is that they are confidently wrong in the direction that kills you, and
they are wrong while displaying a name and a percentage. An app's output
is a candidate to go and verify. It is never an answer, and a high
confidence number is not a second opinion.

**What the data shipped with this project can and cannot do.** The file
`data/locales/silverdale_wa/species.json` is a local species list of the
kind in the fourth row above: 122 records for one real place, each
carrying legal status, edibility with its conditions, toxicity, and a
lookalike list with a danger rating and a paragraph on telling them
apart. It is a reference, and it is honest about its own gaps. The other
file this topic names, `data/plants.csv`, is a **cultivation** database:
growth days, water, nutrients, pH and yields for crops you plant on
purpose. It carries no identification characters, no lookalikes and no
toxicity, and nothing in it should be used to decide what a wild plant
is.

## What competence actually looks like, over years

Nobody becomes safe at this by reading harder. The habits below are
practice rather than sourced findings, so treat them as a rule of thumb;
but they are the shape of what the people who are genuinely good at this
did.

**Learn ten plants completely instead of a hundred approximately.** Ten
species you know in every season, at every stage, with every dangerous
lookalike eliminated by name, is a real skill. A hundred you could
recognise in a photograph is a hazard with a large vocabulary. The
shipped records are built for the first kind of knowledge, which is why a
single record runs to several paragraphs.

**Walk the same ground.** The same route, repeatedly, through the year.
The value is not coverage, it is repetition: you meet the same individual
plants at every stage, and you see the transitions that a guide can only
describe. The rosette in February and the bolted stem in June being the
same plant is a fact you learn by watching, not by reading.

**Photograph the same individuals through the season.** Date and
location, and the parts that matter: whole plant, stem, leaf underside,
flower, fruit. This is the mechanism that makes the camas rule work at
scale. A photo file that runs from flower to seed head to winter rosette
is an identification you can still use in November.

**Write down what you could not determine.** "I could not check the stem
because there was not one" is more useful, a year later, than a
confident name. A record that only contains your successes is a record
that has hidden every place you were guessing.

**Get your determinations checked by a person, out loud, in front of the
plant.** Being told you are wrong is the entire point, and it is the one
thing no document can do for you.

## If somebody has eaten something unknown

Act on this immediately. Do not wait to be sure.

1. **Call Poison Control. In the United States, 1-800-222-1222.** It is
   run by America's Poison Centers, it reaches your local poison centre
   from anywhere in the country, and it is free, confidential and
   staffed around the clock. You do not need to be sure anything is
   wrong to call. If the person is unconscious, having seizures, or
   having trouble breathing, call emergency services first.
2. **Keep a sample of the plant, and keep it separate.** Poison control
   will ask what the plant was and which part was eaten. A physical
   sample, or the rest of what was picked, is often the only way to
   answer that. Where somebody has already vomited, MedlinePlus advises
   saving it, because it can help experts identify what medicine will
   help.
3. **Do not induce vomiting.** MedlinePlus is explicit: do not induce
   vomiting unless told to by poison control or a health care provider.
   Nor should you try to neutralise the poison with lemon juice, vinegar
   or anything else, or use any cure-all antidote, or give anything by
   mouth to somebody who is unconscious.
4. **Do not wait to see how it goes.** This is the one that costs lives,
   and it is on MedlinePlus's do-not list in those words: do not wait for
   symptoms to develop if you suspect somebody has been poisoned.

**Feeling fine proves nothing, and here is the arithmetic behind that.**
Several of the worst toxins have a symptom-free interval by design. FDA
gives the amatoxin latent period as 6 to 48 hours with no symptoms, and
CDC describes a second phase, 24 to 36 hours in, when the patient appears
to recover, before liver failure arrives on day three to five. The cost
of believing that second phase is measurable: FDA's figures put mortality
at about 10 percent for patients given aggressive support almost
immediately, against 50 to 90 percent for those admitted 60 or more hours
after eating. **The time you spend waiting to see whether you feel ill is
the single largest thing you control about the outcome.**

For plants, USDA's own instruction in cases of water hemlock poisoning is
the same: contact a poison control centre and obtain emergency medical
assistance as quickly as possible.

This is also why none of the above depends on knowing what was eaten.
You do not need an identification to make the call. The call is how you
get one.

## What this guide does not qualify you to do

It does not qualify you to eat anything. It was not trying to.

What it should have left you with:

- **Identification is a positive match on every diagnostic character,
  plus the named elimination of every dangerous twin.** Anything less is
  a guess, and "it looks like" is how people die.
- **A field character usually runs in one direction only.** Before you
  lean on one, know what its absence proves, which is often nothing.
- **The parts that carry the answer are the flower, the fruit, the root,
  the stem and the smell, not the leaf**, and those parts are absent for
  most of the year, which is why identification and harvest are separate
  events joined by a physical marker.
- **The carrot family stays shut** until somebody has taught you in
  person.
- **Mushrooms are a different discipline** with a shorter margin and a
  symptom-free window, and belong to their own guide.
- **No test you can run in the field tells you whether an unknown plant
  is safe**, and the tests that claim to are defeated by delay, by dose,
  by preparation and by accumulation.
- **If in doubt, the answer is always the same, and it is free.** Leave
  it. There is no meal on the other side of this that is worth the other
  outcome.

The two topics that come next, what is edible where you live and what
will hurt you there, are separate guides for a reason. Go to them from
the beginning and let them teach it properly, and let a person teach you
the rest.

## Sources

Grouped by what kind of authority each one is. Government publications
are listed first because they are public domain and can be redistributed
with this guide; the rest cannot.

### United States government (public domain)

- USDA Agricultural Research Service, Poisonous Plant Research
  Laboratory. Water hemlock (*Cicuta douglasii*): that it is the most
  violently toxic plant that grows in North America, that only a small
  amount of the toxic substance is needed to produce poisoning in humans,
  cicutoxin as a violent convulsant acting on the central nervous system,
  the toxin found principally in the tubers, the leaf side veins leading
  to notches rather than tips, the chambered rootstalk holding a brown or
  straw-coloured liquid released when the stem is broken or cut, the
  confusions with poison hemlock, wild parsnips, other herbs and
  medicinal plants, and the instruction to contact a poison control
  center and obtain emergency medical assistance as quickly as possible.
  https://www.ars.usda.gov/pacific-west-area/logan-ut/poisonous-plant-research/docs/water-hemlock-cicuta-douglasii/
- USDA Agricultural Research Service, Poisonous Plant Research
  Laboratory. Poison hemlock (*Conium maculatum*): that all parts
  (leaves, stem, fruit and root) are poisonous; coniine,
  gamma-coniceine and related piperidine alkaloids; the three documented
  confusions, hemlock root for wild parsnips, hemlock leaves for parsley,
  hemlock seed for anise; leaves delicate, like parsley; the hollow stem
  marked with small purple spots; the white taproot; height 2 to 3
  metres; habitat along fence lines, irrigation ditches and other moist
  waste places; and that whistles made from the hollow stems have caused
  death in children.
  https://www.ars.usda.gov/pacific-west-area/logan-ut/poisonous-plant-research/docs/poison-hemlock-conium-maculatum/
- USDA Natural Resources Conservation Service. Plant Guide: Meadow
  Deathcamas, *Zigadenus venenosus* (PLANTS symbol ZIVE). That the common
  name refers to the toxicity of the plant and its similarity in
  appearance to camas; that it has been mistaken for other edible bulbous
  plants such as wild onion, sego lily and camas, especially when flowers
  are lacking; that eating one or two bulbs is enough to cause severe
  illness in children and 4 or 5 can cause death; that signs of poisoning
  can begin several hours to a day after ingestion; the creamy white
  petals a quarter inch long with a large gland at the base; and that
  consumption has been linked to deaths of livestock and humans.
  https://plants.sc.egov.usda.gov/DocumentLibrary/plantguide/pdf/pg_zive.pdf
- USDA Forest Service, Celebrating Wildflowers. Common camas,
  *Camassia quamash*: flowers blue to bluish-violet with tepals 12 to 35
  millimetres (0.5 to 1.4 inches) long, and bulbs steamed or pit cooked
  for one to three days, with a full one third of a bulb's cooked weight
  becoming fructose when prepared that way.
  https://www.fs.usda.gov/wildflowers/plant-of-the-week/camassia_quamash.shtml
- USDA Forest Service. Fire Effects Information System, species review:
  *Zigadenus venenosus*, meadow deathcamas. Zygacine as a neurotoxic
  steroidal alkaloid; that livestock and wildlife are poisoned by the
  bulbs, stems, leaves, flowers and seeds; and that dried meadow
  deathcamas remains toxic for at least 20 years.
  https://www.fs.usda.gov/database/feis/plants/forb/zigven/all.html
- USDA Forest Service. Fire Effects Information System, species review:
  *Actaea rubra*, red baneberry. A deciduous perennial herb usually 1 to
  3 feet tall; fruits showy, poisonous, red or occasionally white; a
  poisonous essential oil or glycoside (protoanemonin) found in all parts
  but most concentrated in the berries and root; and that the berries are
  unpalatable and can cause illness to people eating them.
  https://research.fs.usda.gov/feis/species-reviews/actrub
- USDA Forest Service, Celebrating Wildflowers. Claspleaf twistedstalk,
  *Streptopus amplexifolius*: only one flower on each flower stalk,
  hanging downward, giving rise to a single elliptic yellow or red berry
  which is edible; and the warning that the species can be mistaken for
  the poisonous *Veratrum*, so perhaps it is better left unsampled.
  https://www.fs.usda.gov/wildflowers/plant-of-the-week/streptopus_amplexifolius.shtml
- USDA Forest Service, Northern Research Station. Ostry, M.E., Anderson,
  N.A. and O'Brien, J.G., *Field Guide to Common Macrofungi in Eastern
  Forests and Their Ecosystem Functions*, General Technical Report
  NRS-79, 2011. That many mushrooms can be identified only by examining
  the colour of spore prints or spores and tissues under a microscope;
  that changes with age make it necessary to examine several individuals;
  the *Amanita* signature of a ring on the stalk, a cup (volva) at the
  base often within the soil layer, white gills free from the stalk and a
  white spore print; that mushrooms should always be dug, not picked, in
  order to detect the cup; that *Amanita virosa*, *A. verna* and
  *A. bisporigera* can only be distinguished from each other by their
  spore characteristics and collectively cause 95 percent of fatal
  mushroom poisonings; and the instruction repeated on every page, do not
  eat any mushroom unless you are absolutely certain of its identity.
  https://www.nrs.fs.usda.gov/pubs/gtr/gtr_nrs79.pdf
- US Food and Drug Administration. *Bad Bug Book: Foodborne Pathogenic
  Microorganisms and Natural Toxins*, second edition, chapter on mushroom
  toxins. That for individuals who are not trained experts there are
  generally no easily recognizable differences between poisonous and
  nonpoisonous species; that folklore notwithstanding there is no
  reliable rule of thumb for distinguishing edible mushrooms from
  poisonous ones; that most mushrooms causing human poisoning cannot be
  made nontoxic by cooking, canning, freezing or any other means of
  processing; the amanitin latent period of 6 to 48 hours (average 6 to
  15) during which the patient shows no symptoms, death in 50 to 90
  percent of cases, and mortality of about 10 percent with aggressive
  support almost immediately against 50 to 90 percent for those admitted
  60 or more hours after ingestion; that poisonings are almost always
  caused by wild mushrooms collected by nonspecialists, although
  specialists also have been poisoned; that intoxication has occurred
  when people relied on folk methods of distinguishing poisonous from
  safe species; the recorded illnesses from raw, stir-fried, home-canned,
  blanched and frozen mushrooms and mushrooms cooked in tomato sauce,
  which can render the sauce itself toxic even when no mushroom is eaten;
  the four routes by which poisonings occur in the US; and the consumer
  summary noting that many cases have happened in people who were using
  field guides, had a lot of experience, and were sure they had picked
  the right kind. https://www.fda.gov/media/83271/download
- Centers for Disease Control and Prevention. Vo, K.T., et al., Amanita
  phalloides Mushroom Poisonings, Northern California, December 2016,
  *MMWR* 66(21), 2017. The three phases of amatoxin poisoning and their
  timing, that patients evaluated early may be discharged home only to
  return later with indications of liver failure, the case fatality rate
  of 10 to 20 percent, and the outcomes of the 14-case cluster including
  three liver transplants and one child left with permanent neurologic
  impairment. https://www.cdc.gov/mmwr/volumes/66/wr/mm6621a1.htm
- National Library of Medicine, MedlinePlus. Poisoning first aid. The
  Poison Help hotline on 1-800-222-1222; and the do-not instructions:
  do not induce vomiting unless told to by the Poison Control Center or a
  health care provider, do not try to neutralise the poison, do not use
  any cure-all antidote, do not give an unconscious person anything by
  mouth, and do not wait for symptoms to develop if you suspect someone
  has been poisoned. Also that where somebody has been made ill by a
  plant part, saving the vomit may help experts identify what medicine
  can be used. https://medlineplus.gov/ency/article/007579.htm
- National Library of Medicine, MedlinePlus. Pokeweed poisoning, read for
  the information the poison centre asks for on a plant call: the
  person's age, weight and condition, the time it was swallowed, the
  amount, and the name and part of the plant that was eaten, if known.
  Also the same instruction not to make a person throw up unless told to
  by poison control or a health care provider.
  https://medlineplus.gov/ency/article/002874.htm
- National Park Service, Mount Rainier National Park. Mushrooms. That
  identifying wild mushrooms can be extremely difficult and consuming
  them can be deadly, that its species descriptions are intentionally
  brief and are not sufficient to positively identify edible mushrooms,
  and that you should not collect or eat any mushroom unless you are 100
  percent confident of its identification.
  https://www.nps.gov/mora/learn/nature/mushrooms.htm
- National Park Service, Alaska. Poisonous plants. Baneberry berries
  being poisonous and often sending the heart into cardiac arrest; death
  camas effects appearing 1 to 8 hours after eating, with excessive
  salivation and burning and numbness of the lips and mouth; and the
  general rule given there, not to eat anything in the wild unless you
  can positively identify it without question.
  https://www.nps.gov/anch/planyourvisit/poisonous-plants.htm
- National Library of Medicine, NCBI Bookshelf. Horowitz, B.Z. and Moss,
  M.J., *Amatoxin Mushroom Toxicity*, StatPearls. That amanitin is heat
  stable and remains toxic whether eaten raw or cooked, and that the
  symptom-free interval runs for at least 6 hours with laboratory signs
  of liver injury taking about 24 hours to appear. Distributed under
  CC BY-NC-ND 4.0, so cited rather than copied.
  https://www.ncbi.nlm.nih.gov/books/NBK431052/

### Peer-reviewed literature

- Long, K., Townesmith, A., Overmiller, A., Applequist, W., Scalzo, A.,
  Buchanan, P. and Bitter, C.C. Plant identification applications do not
  reliably identify toxic and edible plants in the American Midwest.
  *Clinical Toxicology (Philadelphia)*, volume 61, issue 7, 2023, pages
  524 to 528, DOI 10.1080/15563650.2023.2237282, PMID 37535032. Four
  applications tested against 16 species; genus-level accuracy 76 percent
  and species-level accuracy 58 percent, ranging from 94 percent to 34
  percent across applications; five of eleven potentially toxic species
  identified as an edible species by at least one application; and the
  conclusion that the apps cannot be used to safely identify edible
  plants and that foragers require adequate botanical knowledge. Record
  read via the NLM E-utilities abstract endpoint.
  https://pubmed.ncbi.nlm.nih.gov/37535032/

### State agencies and other copyrighted authorities (cited, not reproduced)

State works are not covered by the federal public-domain rule, so these
are cited for the fact and the sentences here are ours.

- Washington State Noxious Weed Control Board. Poison hemlock: a very
  tall biennial up to about 12 feet, hollow hairless stems with
  noticeable purple blotches, fern-like toothed finely divided leaves
  with a strong odour when crushed, toxins remaining potent in dried
  plant material, its resemblance to wild carrot, and its Class B
  listing. https://www.nwcb.wa.gov/weeds/poison-hemlock
- Washington State Noxious Weed Control Board. Wild carrot: that it may
  be confused with poison hemlock although it is smaller and lacks purple
  blotches on the stems, that the whole plant is covered with coarse
  stiff hairs, its height of 1 to 4 feet, its flat-topped umbels 2 to 4
  inches across often with purple or pinkish flowers in the centre, and
  its Class C listing. https://www.nwcb.wa.gov/weeds/wild-carrot
- America's Poison Centers. Poison Help. The 1-800-222-1222 number, that
  it reaches the caller's local poison centre from anywhere in the United
  States, and that the service is free, confidential and available at all
  hours. https://www.poisonhelp.org/

### Could not be opened for this guide

Listed so nobody assumes they were consulted.

- US Army Field Manual FM 21-76, the usual attribution for the universal
  edibility test. The archived scan located for this document has no text
  layer, so the procedure is described above as it commonly circulates
  rather than quoted, and the description is labelled as such in the
  text.
- USDA Forest Service, Pacific Northwest Research Station, the Pacific
  Northwest chanterelle report (PNW-GTR-576). Its text could not be
  extracted from the PDF in this session, so the Paxillus syndrome
  accumulation point above is attributed to the shipped species record
  that cites it, not quoted from the report directly.
  https://www.fs.usda.gov/pnw/pubs/pnw_gtr576.pdf

### The locale data this guide's worked examples rest on

The three worked pairs, the catch-all lookalike pattern, the onion-smell
asymmetry, the knotweed mark-and-return rule and the honest
"a beginner cannot reliably do this" verdicts all come from
`data/locales/silverdale_wa/species.json`, which records its own source
URLs per record and its own licence at the top of the file. Its
provenance is US federal sources plus Washington State agencies restated
in our own words, with nothing taken from GBIF, iNaturalist, Wikipedia or
OpenStreetMap. The record shape is defined in `schemas/locale.toml`.
`data/plants.csv` is a cultivation database and carries no identification
or toxicity data; it is named here only to say what it is not.

### Companion guides in this collection

- [The Species Where You Live](the_species_where_you_live.md)
- [The Hazards Your Place Actually Has](the_hazards_where_you_live.md)
- [Where You Are: Silverdale, Washington](../locale/silverdale_wa.md)
- [The Growing Calendar](the_growing_calendar.md)
