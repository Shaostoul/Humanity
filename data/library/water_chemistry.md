# Water Chemistry

Take the lid off a kettle that has been boiled every day for a year and
look at the element. There is a hard pale crust on it. Scrape a bit off
with a fingernail; it is gritty, like very fine stone.

That is stone. It is the same mineral as chalk and limestone, and an
hour before it landed on your element it was invisible, dissolved in
water that looked exactly like water. Nobody added it. You boiled clean
tap water and rock came out.

This guide is about why that happens, and about the dozen other things
that follow from the same handful of facts.

## What this guide is for, and what it cannot do

Four guides in this Library already tell you where water comes from,
where it goes, how to test it and how to make it safe. (There are others
alongside them on catching rain, storing it and reusing it, but those
four are the ones this guide sits underneath.) Every one of them hands
you numbers: 10 mg/L, 0.3 mg/L, pH 6.5 to 8.5, 500 mg/L. They tell you
which side of each line you want to be on.

None of them tells you what those numbers physically are. So a reader
finishes all four knowing which thresholds matter and still unable to
predict anything. Why does one well stain the bath orange and the
neighbour's does not? Why does a softener help the kettle and do nothing
for the nitrate? Why does the county say run the tap before you drink,
and then say do not run the tap before you sample?

This guide is the layer underneath. Read it and those four stop being
lists of thresholds. You should be able to look at an unfamiliar
laboratory sheet, or an unfamiliar product on a shelf, and reason about
it rather than look it up.

**Two honest limits.** First, nothing here tells you what is in your
water. Chemistry is not a substitute for a test, and
[Testing Water](/library#testing-water) explains why your senses are very
nearly useless for this. Second, this guide does not teach treatment
procedure. [Making Water Safe to Drink](/library#making-water-safe-to-drink)
is where boiling times, bleach doses and filter pore sizes live. What
this guide adds is the reason each of those works on what it works on,
which is the part that lets you evaluate a product nobody has reviewed.

Where this guide leans on a number one of the others already carries, it
points at that guide rather than reprinting it. The point is to complete
them, not to duplicate them.

## The one idea underneath everything: water is a lopsided molecule

Almost everything in this guide falls out of a single structural fact.

USGS states it plainly: "Water molecules have a polar arrangement of
oxygen and hydrogen atoms." The hydrogen side, USGS says, carries a
positive electrical charge and the oxygen side a negative one. Water is
not a neutral little ball. It is a molecule with a positive end and a
negative end, and that makes it behave like an extremely weak magnet
with respect to anything else that carries charge.

Watch what that does to table salt. USGS again: "Water can become so
heavily attracted to a different compound, like salt (NaCl), that it can
disrupt the attractive forces that hold the sodium and chloride in the
salt compound together." The mechanism is exactly what you would guess
from the shape: "The positively-charged side of the water molecules are
attracted to the negatively-charged chloride ions and the
negatively-charged side of the water molecules are attracted to the
positively-charged sodium ions," and so "Water molecules pull the sodium
and chloride ions apart, breaking the ionic bond that held them
together."

That is dissolving. A crystal of salt is not destroyed or converted; it
is taken apart piece by piece by billions of small electrical tugs, and
the pieces are then surrounded by water molecules and carried away
individually. The record `sodium_chloride` in
`data/chemistry/compounds.csv` gives salt's solubility as 360 g/L, which
is an enormous amount of rock to hide in a litre of something clear.

This is why USGS calls water "the 'universal solvent'" and says it "is
capable of dissolving more substances than any other liquid." And it is
why the same page adds the qualifier that matters: "Of course it cannot
dissolve everything."

**The rule that predicts which is which.** Water dissolves things that
carry charge, or that have their own lopsided positive and negative
ends. It does very badly with things that are electrically bland all
over. Chemists shorten this to "like dissolves like."

Look down the `solubility_water` column in `compounds.csv` and the split
is visible without knowing any chemistry. The salts and the acids and
the small alcohols are soluble or miscible: `sodium_chloride` 360 g/L,
`potassium_chloride` 340 g/L, `saltpeter` 316 g/L, `ethanol` miscible,
`acetic_acid` miscible, `glucose` 909 g/L. The greasy hydrocarbons are
not: `octane` insoluble, `kerosene` insoluble, `toluene` 0.52 g/L,
`benzene` 1.79 g/L. Sugar and alcohol have oxygen and hydrogen sticking
out with charge on them; petrol is carbon and hydrogen arranged so
evenly that water has nothing to grab.

A caveat about where that last explanation comes from, because it
applies to a few other places in this guide as well. The polarity of
water, the positive and negative ends, and the salt mechanism are all
stated by USGS on the pages cited above. The generalisation to "like
dissolves like," the hydrogen bond, which is the specific name for the
attraction between the positive hydrogen of one water molecule and the
negative oxygen of the next, and the structure of a soap molecule
described below are standard chemistry but are **not** on any page
opened for this guide. USGS describes the attraction without naming it.
Treat that vocabulary as ordinary textbook material rather than as
something a federal agency said.

### Four things that follow immediately

**1. Hardness exists.** Rain falling through air picks up carbon
dioxide, which makes it a weak acid, and weak acid dissolves carbonate
rock. The National Park Service describes the process that carves caves:
"precipitation, such as rainwater or snowmelt, mixes with carbon dioxide
from air and decaying plants in soil and forms carbonic acid," and "Once
the acidic water reaches carbonate rocks (e.g. marble, limestone,
dolomite), it can seep into cracks and dissolve the rock to create rooms
and passageways." A cave is a hole where the water took the rock away
dissolved. The calcium is still in the water, downstream, and eventually
in your kettle.

**2. Salt wrecks soil.** Sodium dissolves completely and goes wherever
the water goes, including into the spaces between clay particles, where
it does structural damage that has nothing to do with toxicity.
[Greywater](/library#greywater) carries that argument in full, with its own
sources, including the sodium adsorption ratio and why softener brine is
prohibited from greywater systems by name. It is the best worked example
in the Library of a dissolved thing causing harm by physics rather than
poisoning.

**3. Oil does not rinse off.** Water has nothing to grip on a fat or a
grease, so it beads and runs past. Soap works because a soap molecule is
built as a compromise: a greasy tail that will bury itself in the oil
and a charged head that water will accept. That is also why hard water
ruins soap, which is the next section but one.

**4. And the big one: a filter cannot remove something that is
dissolved.** This is the single most useful consequence in the guide, so
it gets its own section.

### Dissolved versus suspended, and why the difference decides everything

Two things can be "in" water, and they are not the same kind of thing at
all.

**Suspended** means small solid pieces, floating or drifting, still
themselves. Silt, clay, rust flakes, leaf fragments, bacteria, cysts.
They are separate objects with edges. They will eventually settle if you
leave the water alone. They scatter light, which is why water carrying
them looks cloudy.

**Dissolved** means taken apart and mixed in among the water molecules,
one particle at a time, with no edges and nothing to settle. Salt,
calcium, nitrate, lead, arsenic, sugar, oxygen. Dissolved things do not
settle out, and they are very much smaller than anything a filter is
built to catch. For scale:
[Making Water Safe to Drink](/library#making-water-safe-to-drink) records
CDC's figure of about 0.01 micron for the finest class of filter, the
ultrafilter, and a dissolved ion is far below even that.

Most dissolved substances are also invisible, which is the property that
matters here, though that one has an exception worth naming. USGS's list
of what makes water turbid includes dissolved coloured organic
compounds, which are the kind of thing that stains a bog stream brown.
So a few dissolved things do colour water. What none of them do is make
it cloudy in the
way suspended particles do, and the great majority of what this guide is
about, every ion and every gas, is completely invisible at any
concentration that matters.

A filter is a sieve. It works by having holes smaller than the thing you
want to keep out.
[Making Water Safe to Drink](/library#making-water-safe-to-drink) gives CDC's
ladder of pore sizes and which organism each catches, and the whole
ladder lives in the suspended world. Nothing on it can sieve out a
dissolved ion, because a dissolved ion is travelling in among the water
molecules at roughly their own scale. Any hole big enough to let the
water through is big enough to let the sodium through with it. You
cannot separate them by size alone.

So anything that removes a dissolved substance has to work by some
principle other than sieving: sticking it to a surface, swapping it for
a different ion, pushing water through a membrane and leaving it behind,
or boiling the water away from it. Every one of those has a different
blind spot, and that is what the treatment section at the end of this
guide is about.

Carry this one sentence out of here if nothing else: **clarity is
mostly a statement about what is suspended, and it tells you almost
nothing about what is dissolved.**
[Where Water Comes From](/library#where-water-comes-from) makes the same point
from the safety side and calls it the most expensive wrong belief about
water. This is the physical reason it is true.

## pH

pH is the measurement everybody has heard of and almost nobody can use,
because two things about it are routinely skipped.

USGS gives the definition: "pH is a measure of how acidic/basic water
is." The scale "goes from 0 to 14, with 7 being neutral," with lower
numbers acidic and higher numbers basic.

### It is logarithmic, and that is not a technicality

USGS: "pH is reported in 'logarithmic units'. Each number represents a
10-fold change in the acidity/basicness of the water." Their example:
"Water with a pH of five is ten times more acidic than water having a pH
of six."

People nod at this and then reason about pH as if it were a thermometer,
where 6 and 7 are neighbours. They are not neighbours. They are a factor
of ten apart.

Work it through with numbers you will actually meet:

- EPA's recommended range for drinking water runs from pH 6.5 to pH 8.5.
  That is two units, so the acceptable range spans a **hundredfold**
  difference in acidity from one end to the other. Two supplies can both
  be "in range" and be a hundred times apart.
- USGS says "Normal rainfall has a pH of about 5.6", slightly acidic,
  and attributes that to carbon dioxide gas from the atmosphere. That
  is 1.4 units
  below neutral, so ordinary clean rain is about **25 times** more
  acidic than neutral water. Rain is not neutral anywhere, and it never
  was.
- USGS's own example of coal mine drainage sits at pH 2, which is five
  units below neutral: a hundred thousand times.

pH belongs to a family of scales that do not behave the way a ruler
does, and the habit of checking which kind you are holding is worth
having generally.
[Units and Converting Them](/library#units-and-converting-them) works through
the two other members you will meet: temperature, which has an offset as
well as a scale factor, and area and volume, which go as the square and
the cube of a length. It does not cover logarithmic scales, so pH is the
third kind and this is the place it gets explained.

### What pH does not tell you, which is the amount

Here is the part most popular writing omits entirely, and it is the
reason a pond, a soil, or a body of water resists change.

pH tells you the **intensity** of the acidity right now. It says nothing
about **how much acid or base it would take to move it.** Those are
different questions, and the second one has its own measurement.

That measurement is **alkalinity**, and USGS defines it as exactly this:
"The buffering capacity of a water body; a measure of the ability of the
water body to neutralize acids and thus maintain a fairly stable pH
level."

Two things about alkalinity trip people up.

**It is not a chemical.** USGS: alkalinity is "a property of water that
is dependent on the presence of certain chemicals in the water, such as
bicarbonates, carbonates, and hydroxides." You cannot buy a bottle of
alkalinity. It is a capacity that a particular mixture of dissolved
substances happens to have.

**It is not the same word as "alkaline".** Alkaline means the pH is
above 7. Alkalinity means the water has reserves that resist the pH
moving at all, in either direction. Water can be slightly acidic and
have high alkalinity; water can be slightly basic and have almost none.

USGS measures it by titration: add acid in small increments while
watching the pH. At first almost nothing happens, because the
bicarbonate and carbonate are absorbing each addition. Then the reserves
run out and the pH drops steeply. The volume of acid it took to reach
that turn is the alkalinity.

**Where it comes from is geology.** USGS notes that runoff from
limestone country raises alkalinity, while granite country produces
lower alkalinity. Limestone is calcium carbonate; it puts carbonate and
bicarbonate into every drop that touches it. Granite does not.

**The consequence.** USGS: water with high alkalinity "will experience
less of a change in its own acidity when acidic water, such as acid rain
or an acid spill, is introduced into the water body." Put that together
with the geology above and you get the useful prediction: two lakes can
take the same rainstorm, or the same spill, and respond completely
differently, and the thing that decides which is which is the rock their
water came off rather than anything about the lakes themselves.

An analogy that holds up: pH is the reading on the thermometer;
alkalinity is the size of the radiator. Knowing the temperature of a
room tells you nothing about how fast it will cool when you open a
window.

That distinction is not an academic nicety. It comes back twice in this
guide: it is half of what decides whether your plumbing dissolves into
your water, and it is the reason a soil or a pond has a characteristic
resistance to whatever you add to it.

### What a pH number is good for at home

Low pH is the one that costs you money, and not by hurting you. EPA's
secondary standard for pH is a range, 6.5 to 8.5, and EPA lists the
noticeable effects on both sides: at low pH, "bitter metallic taste;
corrosion"; at high pH, "slippery feel; soda taste; deposits." Neither
of those is a health effect, and EPA says of the whole secondary list
that "These contaminants are not considered to present a risk to human
health at the SMCL."

Acidic water is not a poison. It is a solvent that has been made
slightly hungrier, and the thing it is going to dissolve is your
plumbing. That is the corrosivity section below, and it is the reason pH
earns its place on the annual well test that
[Testing Water](/library#testing-water) describes.

## Hardness

WHO's definition is the original one and it is the most useful, because
it names the thing people actually notice. WHO describes hardness as the
traditional way of measuring how much water reacts with soap, with hard
water needing considerably more soap before it will lather at all.

USGS gives the modern version: "The simple definition of water hardness
is the amount of dissolved calcium and magnesium in the water."

Both are right, and the gap between them is the story. Hardness was
defined by laundry a long time before anybody could measure calcium.

### Where it comes from

USGS: "As water moves through soil and rock it dissolves small amounts
of naturally-occurring minerals and carries them into the groundwater
supply." WHO names the rocks: calcium and magnesium occur in many
sedimentary rocks, of which limestone and chalk are the commonest.

So hardness is a map of the geology upstream of you, delivered in
solution. Limestone and chalk country gives hard water. Granite country,
which USGS names as the low-alkalinity case, gives soft water. The same
rock that sets alkalinity sets hardness, which is why the two usually
move together.

The scale of this is easy to underestimate. USGS reports that "In the
United States, about 40% of the groundwater used for drinking comes from
karst aquifers," karst being terrain "created from the dissolution of
soluble rocks, principally limestone and dolomite." Two fifths of the
groundwater Americans drink comes out of rock that water has been
eating. Note the qualifier: that figure is about drinking supply, not
about all groundwater withdrawn for every purpose.

USGS adds the reason groundwater is usually harder than surface water:
"Because water is such an excellent solvent it can contain lots of
dissolved chemicals. And since groundwater moves through rocks and
subsurface soil, it has a lot of opportunity to dissolve substances as
it moves. For that reason, groundwater will often have more dissolved
substances than surface water will." Contact time is the variable. A
creek touches rock for hours; an aquifer holds water against rock for
years.

WHO adds a detail worth knowing because it explains odd test results. No
single substance causes hardness, WHO says. It comes from a range of
dissolved metal ions carrying more than one positive charge, mostly
calcium and magnesium, but with aluminium, barium, iron, manganese,
strontium and zinc all contributing as well. So iron and manganese count
toward hardness, which is one reason a high-iron well can report hardness
that the calcium alone does not explain.

### The bands, and what they are not

USGS and WHO agree on the classification, in milligrams per litre
expressed as calcium carbonate: below about 60 is soft, 60 to 120
moderately hard, 120 to 180 hard, and above 180 very hard.
[Testing Water](/library#testing-water) reprints the bands in the context of
reading a laboratory sheet, which is where you will meet them.

What the bands are not is a safety scale. USGS: "Hardness is a property
of water that is not a health concern, but it can be a nuisance." Very
hard water is not dangerous water. It is expensive water.

You may also see hardness in **grains per gallon**, which is the unit
the softener industry uses and the one printed on the dial of most
equipment. Penn State Extension gives the conversion: one grain per
gallon is about 17 mg/L. So the very-hard threshold of 180 mg/L is
roughly 10.6 grains per gallon. Any time a salesman and a laboratory
seem to disagree by a factor of about seventeen, this is why.

### Temporary and permanent hardness, and why a kettle furs up

WHO notes that although hardness is caused by those positive ions, it
can also be discussed in two parts: carbonate hardness, which it gives
the older name of temporary hardness, and non-carbonate hardness, older
name permanent. Those two old names describe genuinely different
behaviour and they explain the crust you scraped off the element.

**Carbonate hardness is the calcium and magnesium that arrived paired
with carbonate and bicarbonate**, which is to say the part that came out
of limestone, and it is the part that is also providing your alkalinity.
It is held in solution by dissolved carbon dioxide. Take the carbon
dioxide away and the calcium has nothing to stay dissolved with, so it
drops out as solid calcium carbonate.

That is not a metaphor; it is the same reaction that builds caves in
reverse. The National Park Service describes a drop of cave water
hanging from a ceiling: "With the loss of carbon dioxide, a thin film of
carbonate material precipitates," and where the drop lands, "carbon
dioxide is given off and carbonate material is precipitated as a mound
below the point of dripping." A stalactite is limestone that dissolved
somewhere uphill, travelled in solution, and came back out when the
water let go of its carbon dioxide.

Heating water does the same thing much faster, because gases come out of
hot water. USGS: "When hard water is heated, such as in a home water
heater, solid deposits of calcium carbonate can form. This scale can
reduce the life of equipment." Your kettle element is a very small,
very fast stalactite.

**Non-carbonate hardness is the calcium and magnesium that arrived
paired with sulfate or chloride instead**, and it does not do this,
because those salts are far more soluble. The numbers in
`data/chemistry/compounds.csv` show the size of the gap plainly:
`calcium_carbonate` has a listed water solubility of 0.013 g/L, while
`gypsum`, which is calcium sulfate, is listed at 2.0 g/L. That is about
150 times more soluble. Driving off carbon dioxide will not make calcium
sulfate precipitate, because calcium sulfate was never depending on
carbon dioxide to stay in solution.

Hence the old names. Boiling removes the carbonate share, so it was
called temporary. Boiling does nothing to the rest, so that was called
permanent.

### What hardness actually costs you

Three things, and only one of them is expensive.

**Scale, which is the real cost.** USGS: "Hard water can cause mineral
buildup in plumbing, fixtures, and water heaters." WHO is specific about
the mechanism that hurts. Wherever water is heated, insoluble metal
carbonates form, and WHO's account is that they coat the surfaces and
cut the efficiency of heat exchangers. A layer of stone between
your heating element and your water is an insulator sitting exactly
where you need conduction. It makes the element work harder, run hotter,
and fail sooner. Kettles, water heaters, dishwashers, boilers, and any
heat exchanger in a solar or heat-pump system are all in this category.
If hardness ever justifies spending money, this is why.

**Soap that will not lather.** USGS: "In hard water, soap reacts with
the calcium (which is relatively high in hard water) to form 'soap
scum'." WHO describes the same thing as a visible deposit left in
containers, made of insoluble metals, soaps or salts, and says the
familiar bathtub ring is an example of it. WHO also records that hard
water pushes up how much soap gets used.

The chemistry behind that curd is worth understanding once, and it is
textbook chemistry rather than something an agency page opened for this
guide spells out, so take it as background. Soap is the sodium salt of a
fatty acid;
`stearic_acid` in `compounds.csv` is one of the fatty acids soap is made
from, and its record names soap production among its uses. Sodium soaps
dissolve. Calcium, though, carries two positive charges where sodium
carries one, so calcium can hold two soap molecules at once, and that
pairing is not soluble. It leaves solution as a grey curd, which is the
ring on the bath and the dullness in a towel. Every calcium ion in the
water takes soap out of circulation before any of it reaches the dirt,
which is exactly why hard water swallows soap.

What is sourced here is the outcome rather than the mechanism: USGS says
soap reacts with calcium to form soap scum, and WHO says hard water
requires considerably more soap to produce a lather and leaves a
deposit. The charge arithmetic is the standard explanation for why.

**And a third thing that is not a cost.** WHO's fourth-edition
conclusion is that there is not enough data to name either a minimum or
a maximum mineral concentration, and so it proposes no guideline value
at all. There is no health guideline for hardness because there is no
established health effect to set one against.

It is worth stating carefully what is contested rather than flattening
it. WHO reports that a large number of studies have found an inverse
relationship between water hardness and cardiovascular mortality, but
that most of them are ecological studies, a design whose built-in
weaknesses restrict what can be concluded from them. Better-designed
case-control and cohort studies found no association between total
hardness or calcium and heart disease, and a mixed and unresolved
picture for magnesium. WHO's own summary is that the evidence remains
under debate and does not establish cause. On eczema, WHO reports only
that hard water has been suggested as something that might make eczema
worse, with a proposed mechanism of soap residue left on skin, some
reported associations in primary-school children but not secondary, and
studies still under way. Both of these are open questions, not findings,
and anybody selling you equipment on the strength of either is ahead of
the evidence.

WHO does note the other direction: drinking water typically supplies 5
to 20 percent of total calcium and magnesium intake, so water is a real
if minor dietary source of both, and stripping it out entirely is a
small loss rather than a gain.

### What a softener does, and the thing it adds

An ion exchange softener does not remove hardness. It trades it.

WHO gives the mechanism in one line: every doubly charged ion in the
water, a calcium or a magnesium, leaves and two sodium ions take its
place. Penn State
Extension describes the hardware: water passes through a bed of resin
beads coated in sodium; calcium and magnesium stick to the resin and
sodium comes off into the water. When every site on the resin is
occupied the bed is spent, and it is recharged by flushing it with
strong salt brine, which drives the calcium and magnesium off and sends
them down the drain. Penn State puts the water cost of one regeneration
cycle at about 50 gallons.

Two consequences follow, and both matter.

**Consequence one: softened water carries added sodium.** Penn State
gives the rate: for every grain per gallon of hardness the unit takes
out, it puts 7.5 milligrams of sodium into each quart.

Put a number on that. Take very hard water at 180 mg/L, which is about
10.6 grains per gallon. At 7.5 mg per quart per grain, softening it adds
roughly 79 mg of sodium to a quart, so about 160 mg in two quarts, which
is close to two litres. FDA gives the Daily Value for sodium as "less
than 2,300 milligrams (mg) per day" and says "Americans eat on average
about 3,400 mg of sodium per day." So drinking two litres of that
softened water a day contributes roughly 7 percent of the Daily Value.

That arithmetic is ours, built on Penn State's rate and FDA's Daily
Value; neither agency published the combined figure. Redo it for your
own hardness before relying on it.

What it means: for most people this is a small addition to a large
number, and the sodium in your food dwarfs it. **For somebody on a
medically restricted sodium intake it is not negligible, and it is a
question for their clinician rather than for a guide.** The honest
framing is that softening moves you along a scale you may already be
being asked to watch.

WHO and Penn State both note the standard workaround: soften only the
hot water line, or bypass the kitchen tap, so the water you drink and
cook with is untouched and the water that would otherwise scale your
heater is treated. This is cheaper as well as better.

**Consequence two: the waste brine is the worst possible water for
soil.** [Greywater](/library#greywater) carries this argument in full and with
its own sources. The short version is that soil structure depends on
calcium and magnesium bridging clay particles together, that sodium
cannot bridge, and that replacing one with the other collapses the
structure. A softener's regeneration waste is by design the highest
sodium, lowest calcium water the house produces, which is precisely the
worst combination. Greywater rules prohibit it by name, and that is not
fussiness. Read that guide before you route any softener discharge
anywhere near ground you care about.

**And the thing a softener is not.** Penn State is blunt about it:
softening should not be expected to remove any of the more serious
contamination problems a drinking water supply can have. It is an
appliance for scale and laundry.
It is not a treatment for anything on the health list, and CDC's home
treatment guidance confirms that softeners do not remove parasites,
bacteria or viruses.

One practical trap worth repeating from [Testing Water](/library#testing-water):
do not draw a water sample from a tap that has a softener on it, because
you will be measuring the appliance instead of the supply.

## Three measurements people conflate: dissolved solids, conductivity, turbidity

These get mixed up constantly, and they answer three different
questions.

### Total dissolved solids

TDS is the total of everything dissolved, added up without distinction.
USGS: "The dissolved solids concentration in water is the sum of all the
substances, organic and inorganic, dissolved in water."

USGS also names the usual suspects: "Calcium, magnesium, sodium,
potassium, bicarbonate, sulfate, chloride, nitrate, and silica typically
make up most of the dissolved solids in water."

Read that list against the rest of this guide and most of it is already
familiar. Calcium and magnesium are hardness. Bicarbonate is alkalinity.
Sodium and chloride are salt. Nitrate is the one with a health limit.
TDS is those things summed into one number, and EPA's secondary standard
for it is 500 mg/L, listed against noticeable effects of "hardness;
deposits; colored water; staining; salty taste."

**What TDS can tell you:** roughly how mineralised the water is, and
whether that has changed since last year. USGS lists the practical
consequences of high dissolved solids as "unpleasant taste, high
water-treatment costs, mineral accumulation in plumbing, staining,
corrosion, and restricted use for irrigation."

**What it cannot tell you:** anything about composition. A TDS of 400
mg/L could be almost entirely calcium bicarbonate from limestone, which
is harmless and scale-forming, or almost entirely sodium chloride from
seawater intrusion, which means your well is failing. Same number,
opposite situations. TDS is a volume knob, not a name.

### Conductivity

Conductivity is a shortcut for measuring TDS, and understanding it as a
shortcut is the whole point.

USGS: "pure water is an excellent insulator and does not conduct
electricity." Electricity moves through water only because dissolved
salts have come apart into "cations (positively charged ions) and anions
(negatively charged ions)," and those charged fragments can carry
current. USGS: "Even a small amount of ions in a water solution makes it
able to conduct electricity."

So a conductivity meter is measuring the total quantity of charged
particles in the water, which in most natural water tracks the total
dissolved solids closely. USGS says so directly: "Total dissolved solids
can be monitored in real time in surface water and groundwater by
measuring its surrogate, specific conductance."

**What conductivity is unbeatable at:** being cheap, instant, and
continuous. A meter costs very little, reads in seconds, needs no
laboratory, and can be read weekly for years. That makes it the right
instrument for watching a trend, which is exactly how
[Testing Water](/library#testing-water) recommends using it on a coastal well.

**What conductivity cannot do:** name anything. Every ion conducts.
Sodium conducts, calcium conducts, nitrate conducts, and the meter adds
them all together into one number with no way to separate them. It also
cannot see anything dissolved that is uncharged, which includes most
organic solvents and dissolved gases. A conductivity reading that jumps
tells you that something ionic changed; it does not tell you what, and
it will sit unmoved while a genuinely dangerous uncharged contaminant
walks past it.

That limitation is a straightforward consequence of the mechanism USGS
describes, but it was not found stated in those words on any page opened
for this guide. Take it as reasoning from the mechanism rather than as
an agency statement.

### Turbidity

Turbidity is the odd one out, because it is mostly a measure of the
suspended world rather than the dissolved one.

USGS: "Turbidity is the measure of relative clarity of a liquid. It is
an optical characteristic of water and is a measurement of the amount of
light that is scattered by material in the water when a light is shined
through the water sample." It is reported "in nephelometric turbidity
units (NTU)," and the things that cause it are "clay, silt, very tiny
inorganic and organic matter, algae, dissolved colored organic
compounds, and plankton and other microscopic organisms."

Because it is a different physical property, it moves largely
independently of the other two. Crystal-clear water can have very high
TDS; muddy snowmelt can have almost none. Note that USGS's list of
causes includes dissolved coloured organic compounds alongside the
particles, so the separation is not perfect, but the useful reading
stands: a turbidity number is telling you about particles, and a
dissolved solids number is telling you about ions.

**Why turbidity matters here is not what it is, but what it shields.**
USGS: "Although turbidity is not a direct indicator of health risk,
numerous studies show a strong relationship between removal of turbidity
and removal of protozoa." And the mechanism: "The particles of turbidity
provide 'shelter' for microbes by reducing their exposure to attack by
disinfectants," and "Turbidity can provide food and shelter for
pathogens."

EPA makes the same point in its primary regulations: "Higher turbidity
levels are often associated with higher levels of disease-causing
microorganisms such as viruses, parasites and some bacteria."

This connects straight to
[Making Water Safe to Drink](/library#making-water-safe-to-drink), which opens
its procedure by telling you to settle and pre-filter cloudy water and
insists that doing so "is not treatment." Now you can see why it is
still mandatory. A particle of silt is a physical hiding place. A
chlorine molecule that reacts with organic matter in the water is a
chlorine molecule that is not available to kill anything, so cloudy
water eats the dose. And as the treatment section below explains, an
ultraviolet lamp is blocked by the same particles, for the same reason a
torch does not shine through mud.

So: TDS and conductivity say how much is dissolved. Turbidity says how
much is floating. And turbidity is the one that decides whether your
disinfection will work.

## Dissolved gases

Water holds gas as well as solids, and three of the gases explain
symptoms people otherwise find mystifying.

### Oxygen

USGS defines dissolved oxygen as "a measure of how much oxygen is
dissolved in the water," which is to say, in USGS's words, "the amount
of oxygen available to living aquatic organisms." It "enters a stream
mainly from the
atmosphere and, in areas where groundwater discharge into streams is a
large portion of streamflow, from groundwater discharge."

**Cold water holds more.** USGS states it flatly: "Cold water can hold
more dissolved oxygen than warm water." The USGS National Field Manual
publishes the actual solubility table, and the numbers make the point
better than the sentence does. In freshwater at sea level pressure, 760
millimetres of mercury, saturation is:

| Water temperature | Dissolved oxygen at saturation |
|---|---|
| 0 degrees C | 14.62 mg/L |
| 5 degrees C | 12.77 mg/L |
| 10 degrees C | 11.29 mg/L |
| 15 degrees C | 10.08 mg/L |
| 20 degrees C | 9.09 mg/L |
| 25 degrees C | 8.26 mg/L |
| 30 degrees C | 7.56 mg/L |

Between freezing and 30 degrees C the water's capacity falls by about 48
percent. Warm water is not slightly worse for fish. It holds a little
over half the oxygen.

**Moving water holds more.** USGS: "Rapidly moving water, such as in a
mountain stream or large river, tends to contain a lot of dissolved
oxygen, whereas stagnant water contains less." Turbulence keeps dragging
the surface under and exposing new water to air, which is the same
principle an aquarium bubbler uses.

**What the numbers mean.** USGS: "The oxygen content of surface waters
of normal salinity in the summer is typically more than 8 milligrams per
liter (8 mg/L); when oxygen concentrations are less than 2 mg/L, the
water is defined as hypoxic."

Put that against the table. A creek at 20 degrees C can hold at most
about 9.09 mg/L, so a healthy summer creek is running close to its
ceiling with very little margin. Now add anything that consumes oxygen.
The USGS field manual names the sinks: "respiration, aerobic
decomposition processes, ammonia nitrification, and other
chemical/biological reactions." Organic matter arriving in the water,
whether from a failed drainfield, a manure pile or fertiliser feeding an
algal bloom, is food, and things that eat it breathe. USGS notes that
"excess organic material in lakes and rivers can cause eutrophic
conditions, which is an oxygen-deficient situation that can cause a
water body to 'die.'"

So the fish-kill mechanism is a squeeze from both ends: warm slow water
has a lower ceiling and the decomposition has a higher demand, and the
two meet in August.

[Where Water Goes](/library#where-water-goes) already names a piece of this
that almost nobody thinks about: rain crossing hot asphalt arrives at a
creek warm, and EPA counts that thermal pollution among the harms of
impervious surfaces. This table is why that is a real harm rather than a
curiosity. A few degrees of warming is a measurable subtraction from the
oxygen the creek can hold.

One small everyday consequence of the same physics: boiled water tastes
flat because heating drove the dissolved air out of it, which is exactly
why [Making Water Safe to Drink](/library#making-water-safe-to-drink) tells
you to pour it between two containers to put the air back.

### Carbon dioxide and the carbonate system

Carbon dioxide is the gas that makes water mildly acidic and then makes
it able to dissolve rock, and it is the hinge of most of this guide.

The sequence runs like this. Rain falls through air and dissolves carbon
dioxide, which is why USGS gives normal rainfall a pH of about 5.6.
Water then moves into soil, where NPS names a second source of the same
gas: rainwater and snowmelt mix "with carbon dioxide from air and
decaying plants in soil." Dissolved carbon dioxide
makes carbonic acid, listed in `compounds.csv` as `carbonic_acid` with
the telling note "only in solution," because unlike most acids you
cannot put it in a bottle. That weak acid dissolves carbonate rock and
carries calcium away. Later, when the water warms or is exposed to air
with less carbon dioxide in it, the gas leaves and the calcium comes
back out as solid carbonate, on a cave ceiling or on your kettle
element.

NOAA describes the same chemistry playing out at ocean scale: "When CO2
is absorbed by seawater, a series of chemical reactions occur resulting
in the increased concentration of hydrogen ions," which "causes the
seawater to become more acidic" and causes "carbonate ions to be
relatively less abundant." More hydrogen ions means lower pH; less
carbonate means shell-building organisms struggle. That is the same set
of species whose flesh concentrates what the creeks deliver, which
[Where Water Goes](/library#where-water-goes) treats as the receipt for a
whole watershed.

The practical point for a household is that the carbonate system is
simultaneously your alkalinity, your hardness, and your scale. They are
not three separate properties that happen to correlate. They are three
views of one equilibrium.

### Hydrogen sulfide

This is the rotten-egg smell, and it is the one dissolved gas in this
guide that kills people.

ATSDR describes it as "a colorless, flammable, highly toxic gas" with "a
characteristic rotten-egg odor," "produced naturally by decaying organic
matter" and "released from sewage sludge, liquid manure, sulfur hot
springs, and natural gas."

In a well it is usually biological. Penn State Extension attributes it
to sulfur-reducing bacteria that feed on sulfur in the water and thrive
in the low-oxygen conditions of wells and plumbing, and notes the
problem is most common in wells drilled into acidic bedrock such as
shale and sandstone. These bacteria are not themselves a health risk;
they just make the gas.

There is a diagnostic detail here that saves people a lot of money.
Penn State explains that water heaters contain a magnesium rod to
protect the tank from corrosion, and that the rod itself can drive a
chemical reduction of sulfate into hydrogen sulfide. **So if only the
hot water smells, the problem is probably your water heater and not
your well**, and Penn State's fix is to swap the magnesium rod for an
aluminium one, which keeps the corrosion protection without the smell.
That is a cheap fix that people routinely skip on the way to buying a
whole-house treatment system.

In the water itself, hydrogen sulfide is an aesthetic problem. Penn
State notes there is no drinking water standard for it because it makes
water undrinkable long before it reaches harmful concentrations, that
most people detect it below 0.5 mg/L, and that it does corrode metals in
plumbing and can leave yellow or black greasy stains where it forms
metallic sulfides. EPA's secondary standards list an odour limit of 3
threshold odour number, with "rotten-egg" named as one of the smells it
is meant to catch.

**But the gas coming out of the water is a different question entirely,
and this is where people die.** ATSDR: "Hydrogen sulfide is slightly
heavier than air and may accumulate in enclosed, poorly ventilated, and
low-lying areas." That is a description of a septic tank, a manure pit,
a sump, a cistern, a sewer and an unventilated well pit.

The hazards, each stated on its own:

**ATSDR: "Inhalation of high concentrations of hydrogen sulfide can
produce extremely rapid unconsciousness and death."** Not illness over
hours. Collapse.

**ATSDR: "Low concentrations (50 ppm) can rapidly produce irritation of
the nose, throat, and lower respiratory tract."** Read "low" as low
relative to what kills, not as low in any ordinary sense. ATSDR puts the
odour threshold at 0.5 parts per billion, so a concentration it calls
low is around a hundred thousand times the concentration at which you
first notice the smell.

**ATSDR: "with continued exposure and at high levels, the poison may
deaden a person's sense of smell."** The smell fading is not the danger
passing. It can be the opposite.

**ATSDR: "Odor is not a reliable indicator of hydrogen sulfide's
presence and may not provide adequate warning of hazardous
concentrations."** Your nose is a detector with no scale on it and a
failure mode that looks like good news.

The rule that follows is simple and absolute: **do not climb into a
septic tank, a manure pit, a cistern, a sump or a well pit, and do not
put your head into one, and this applies most of all when you can smell
that something is down there.**

**And if somebody else is already down there and unconscious, going in
after them is how the second person dies.** That is not emphasis added
here. ATSDR states it about trained responders wearing breathing
apparatus: "Fatalities have occurred to rescuers entering the hot zone,"
and "Rescuers should have a safety line during rescue operations because
of the extremely rapid toxic action of hydrogen sulfide." If
professionals with air supplies and safety lines still die
doing this, a person climbing down a ladder in ordinary clothes has no
chance. Call for help from outside the space and stay outside it.
Reading cannot substitute for confined space training, and this guide is
not attempting to give it.

### Radon

Radon is radioactive gas from the decay of uranium in rock, and it
dissolves in groundwater the way any gas does.

EPA describes the route: "Radon gas can also dissolve and accumulate in
water from underground sources (called ground water), such as wells."
EPA's companion point is that "Radon is not a concern in water that
comes from lakes, rivers, and reservoirs (called surface water), because
the radon is released into the air before it ever arrives at your tap."
This is a groundwater problem only. `compounds.csv` lists `radon_gas`
and its description names it "leading cause of lung cancer after
smoking."

The counterintuitive part is the exposure route. EPA: "When water that
contains radon is used in the home for showering, washing dishes, and
cooking, radon gas escapes from the water and goes into the air." So the
risk from radon in your water is mostly not from drinking it. EPA's
estimate: "radon in drinking water causes about 168 cancer deaths per
year: 89% from lung cancer caused by breathing radon released to the
indoor air from water and 11% from stomach cancer caused by consuming
water containing radon."

Nearly nine tenths of the harm arrives through your lungs from water you
never swallowed. A carbon filter on the kitchen tap does nothing about
that, because the exposure happens in the bathroom.

EPA names two point-of-entry treatments, and they work on opposite
principles: "Granular activated carbon (GAC) filters (which use
activated carbon to remove the radon), and Aeration devices (which
bubble air through the water and carry radon gas out into the atmosphere
through an exhaust fan)." Aeration is the interesting one, because it
does not remove the radon at all; it deliberately transfers it out of
the water and vents it outdoors before it can get into the house air.

EPA's separate guidance for private wells is to test "every three years
for radionuclides." Radon is invisible, has no smell, and is not on the
annual four that [Testing Water](/library#testing-water) describes, so it gets
found only when somebody asks for it.

## Iron and manganese

Orange stains in the bath and black specks in the laundry are among the
complaints that send well owners looking for treatment, and they are
almost always a nuisance rather than a danger. Saying so clearly is the
useful part, because a lot of money gets spent on them. (How common
these are relative to other well complaints is not something any source
opened for this guide quantifies, so no ranking is claimed here.)

USGS gives the origin in one sentence: "As groundwater flows through
sediments, metals such as iron and manganese are dissolved and may later
be found in high concentrations in the water."

EPA's secondary standards set iron at 0.3 mg/L against noticeable
effects of "rusty color; sediment; metallic taste; reddish or orange
staining," and manganese at 0.05 mg/L against "black to brown color;
black staining; bitter metallic taste." Both are on the secondary list,
which EPA does not enforce and which it says does not present a health
risk at those levels. The full table is in
[Testing Water](/library#testing-water).

As for where to expect them, Penn State adds that natural iron and
manganese are more common in deeper wells, where the water has been in
contact with rock longer.

### Why the water is clear at the tap and stains the bath

This is the part that confuses people, and it is pure chemistry.

Underground there is very little oxygen. In that condition iron is in
its reduced form, and reduced iron dissolves. Penn State describes water
containing dissolved iron and manganese as looking colourless when drawn
from the well, because the metals have not yet reacted with oxygen, with
orange-brown or black particles forming after exposure to air as
oxidation occurs.

So the sequence is: the metal comes out of the tap dissolved and
invisible, meets air, changes to its oxidised form, which is not
soluble, and becomes a solid particle. Iron goes orange-brown; manganese
goes black. The stain in the bath is rust that formed after the water
arrived.

Two useful consequences. First, a sample of this water starts changing
the moment it leaves the tap, which is part of why metals samples get
special handling. [Testing Water](/library#testing-water) records the
procedure: a metals bottle arrives already preserved with nitric acid,
and you do not rinse it out. Second, **the oxidation that causes
the problem is also the treatment**: if you deliberately oxidise the
metal first, with air, chlorine or another oxidant, you convert a
dissolved thing into a suspended thing, and a suspended thing can be
filtered. Every serious iron and manganese system is oxidation followed
by filtration.

Penn State's options sort by concentration. An ion exchange softener
will handle dissolved iron below about 5 mg/L but only if the iron is
still in its dissolved form, since oxidised particles foul the resin.
Polyphosphate sequestration works below about 2 mg/L by keeping the
metal in solution so it never forms a stain, which fixes the appearance
without removing anything. Oxidising filters such as manganese greensand
suit roughly 3 to 10 mg/L combined. Above about 10 mg/L combined, Penn
State points to chlorination followed by filtration.

### The honest part

At the levels most wells produce, this is laundry and plumbing, not
health. [Testing Water](/library#testing-water) warns specifically against
spending money treating an aesthetic exceedance, and iron is the
textbook case.

Manganese deserves one qualification. Penn State notes EPA has a
non-enforceable health advisory for manganese of 0.3 mg/L, addressing
possible neurological effects from prolonged exposure at elevated
levels. Compare the two numbers: the aesthetic limit of 0.05 mg/L is six
times stricter than the health advisory. You will see black staining
long before manganese becomes a health question, which is a comfortable
position to be in. But "aesthetic" is not the same as "no number
exists," and if a laboratory reports manganese near or above 0.3 mg/L,
that is a different conversation, particularly where an infant is
drinking the water.

## Nitrate

Nitrate is the one in this guide with a body count, and the chemistry is
what makes it so hard to deal with.

EPA sets the MCLG and the MCL both at 10 mg/L measured as nitrogen, and
states the health effect: "Infants below the age of six months who drink
water containing nitrate in excess of the MCL could become seriously ill
and, if untreated, may die. Symptoms include shortness of breath and
blue-baby syndrome." Nitrite has an MCLG and MCL of 1 mg/L, also as
nitrogen. EPA gives the sources as "Runoff from fertilizer use; leaking
from septic tanks, sewage; erosion of natural deposits."

**The units on that limit are a genuine trap** and
[Testing Water](/library#testing-water) covers it at length. A laboratory can
report nitrate as nitrogen or as the whole nitrate ion, and the two
numbers describe the same water without being the same number. Do not
compare a result to 10 until you know which basis it is on.

### The mechanism

EPA's private well material gives it: "Once taken into the body,
nitrates are converted into nitrites. High levels of nitrate and nitrite
are most serious for infants." And: "High levels of nitrate/nitrite in
drinking water can cause methemoglobinemia or 'blue baby syndrome'."
What that condition is, in EPA's words: "These substances reduce the
blood's ability to carry oxygen. This acute condition can occur rapidly
over a period of days. Symptoms include shortness of breath and blueness
of the skin."

So nitrate is not directly a poison. It is converted in the body to
nitrite, and nitrite attacks the blood's ability to carry oxygen. The
blueness is the visible sign of a child who is not getting oxygen
delivered, in a child who is breathing perfectly well.

**Hazards, each on its own line.**

**The threshold is real and the victim cannot report symptoms.** An
infant under six months cannot tell you they feel wrong, and EPA says
the condition can develop rapidly over days.

**Formula is the exposure route.** Water used to reconstitute infant
formula is water an infant drinks in quantity, every day, at the age of
maximum vulnerability.

**Nitrate has no taste, no smell and no colour at any concentration that
matters.** A clear, cold, sweet well can be over the limit.

**Boiling makes it worse, not better.** Water leaves as steam and the
nitrate stays behind, so boiling concentrates it. This is the exact
opposite of the reflex that protects against germs, and it is why the
chemicals line in
[Making Water Safe to Drink](/library#making-water-safe-to-drink) is, in that
guide's own words, entirely empty.

**If there is a pregnancy or an infant in the house, test.**
[Testing Water](/library#testing-water) lists a new pregnancy and a child
moving in as triggers for an out-of-cycle test, and notes what those
have in common with flooding and well repairs: nothing about the water
changed. What changed is who is drinking it.

### Why nothing simple removes it

Nitrate is a small, singly charged, extremely soluble anion, and every
one of those properties defeats a common treatment.

It is dissolved, so no filter sieves it out. It is not alive, so no
disinfectant touches it. It is not volatile, so boiling leaves it behind
and concentrates it. And it does not stick to activated carbon: CDC's
account of what carbon adsorbs is organic and inorganic chemicals,
chlorine and iodine compounds and most heavy metals, and nitrate is on
none of those lists. The textbook reason is that adsorption works on
things that would rather not be dissolved, and a small, singly charged,
highly water-loving ion like nitrate is perfectly content where it is.
That reason is standard chemistry rather than something an agency page
states, but the practical conclusion does not depend on it: nitrate is
simply absent from every list of what carbon removes.

That leaves three routes, all of which work on entirely different
principles: reverse osmosis, which CDC lists among the chemicals it may
reduce; distillation, which CDC lists nitrate among the chemicals it
removes; and anion exchange, which swaps the nitrate ion for a different
ion the way a softener swaps calcium for sodium.

Note what those three have in common. None of them is a filter, and all
three are point-of-use or whole-house systems with running costs. There
is no camping-kit answer to nitrate.

## Corrosivity, and the lead that comes out of your own pipes

Here is the pathway people get most wrong. Lead is usually not in your
source water. It is in your plumbing, and whether it moves from the pipe
into your glass is decided by the chemistry of the water running through
it.

EPA states the mechanism: "Lead can enter drinking water when plumbing
materials that contain lead corrode, especially where the water has high
acidity or low mineral content that corrodes pipes and fixtures."
Corrosion, EPA says, "is a dissolving or wearing away of metal caused by
a chemical reaction between water and your plumbing."

Note both halves of that condition. **High acidity or low mineral
content.** Low pH is one way to get corrosive water. Simply having very
little dissolved in it is another. Both come back to the same idea: a
solvent that is not already carrying much is a solvent with room, and
water is always looking to dissolve something.

### What makes water corrosive

USGS defines corrosivity as "how aggressive water is at corroding pipes
and fixtures" and names the properties that decide it: "pH, calcium
concentration, hardness, alkalinity, dissolved solids, and temperature."

Read that list. It is this entire guide. Low pH, low hardness, low
alkalinity and low dissolved solids together describe soft acidic water,
and soft acidic water is the corrosive kind.

Which sets up the fact most people find backwards. **Soft water is not
automatically the nicer water.** WHO puts it squarely: soft water that
has not been stabilised is strongly inclined to attack metal surfaces
and pipes, and WHO names the result, heavy metals turning up in the
drinking water, specifically cadmium, copper, lead and zinc. Hard water
costs you an element. Soft unstabilised water can cost you metals in the
glass. Neither extreme is free.

USGS's national assessment put numbers on how common this is. USGS
assessed "more than 20,000 wells nationwide" and found "25 states have
groundwater that has either high or very high potential to be
corrosive," which is half the country. USGS names where: "The states
with the largest percentage of wells with potentially corrosive
groundwater are located primarily in the Northeast, the Southeast, and
the Northwest." About 44 million people in the United States drink from
private wells, and "maintenance, testing and treatment of private water
supplies are the sole responsibility of the homeowner."

USGS also draws the distinction that matters for how you think about it:
"Naturally corrosive water is not dangerous to consume by itself.
Nevertheless, it can cause health-related problems by reacting with
pipes and plumbing fixtures." Corrosivity is not a contaminant. It is a
tendency to create one, out of materials you own.

USGS measures the tendency with two indices, the Langelier Saturation
Index and the Potential to Promote Galvanic Corrosion, the second based
on the ratio of chloride to sulfate. You are unlikely to compute either,
but their existence tells you something: corrosivity is not a single
measured substance, it is a calculation from several of the numbers
already on your sheet.

**Locally.** The Northwest is one of the three regions USGS names. That
is not a coincidence of geology alone: WHO records that rainwater is
soft and, as a rule, a little on the acidic side, and around Silverdale
the ground is
glacial outwash sand and dense till rather than limestone, as
[Where Water Goes](/library#where-water-goes) sets out in detail from the soil
survey. Rain that is already soft and slightly acidic, landing on ground
with little carbonate in it to neutralise it, produces soft, low
alkalinity, mildly acidic water. That is the corrosive combination.

The status of that paragraph needs stating exactly. USGS naming the
Northwest is sourced. WHO on rainwater is sourced. The Silverdale soils
are sourced in the sibling guide. **The conclusion that local water is
therefore typically soft and corrosive is an inference from those
three,** and `data/locales/silverdale_wa/water.json` carries no hardness,
pH or alkalinity figures to check it against. If you are here, get your
own numbers rather than taking this guide's word for it.

### What this means for your tap

EPA: "The most common sources of lead in drinking water are lead pipes,
faucets, and fixtures," along with "brass or chrome-plated brass faucets
and plumbing with lead solder." And: "In homes with lead pipes that
connect the home to the water main, also known as lead services lines,
these pipes are typically the most significant source of lead in the
water."

The variable that decides how much dissolves is **contact time.** EPA:
"The more time water has been sitting in pipes, the more lead it may
contain." Corrosion is a reaction between the water and the metal, and
like most reactions it proceeds as long as the two are in contact.
Moving water is in contact with any given inch of pipe for a fraction of
a second. Water that stood in the pipe overnight was in contact for
eight hours.

**That single fact is why the sampling procedure in
[Testing Water](/library#testing-water) is contradictory-looking and is not.**
To find out what is in your source you flush the line first, because you
want water that has not been sitting. To find out what your plumbing
adds you do the opposite: let it stand undisturbed for hours and collect
the very first water out, because that is the water that had time to
react. Same tap, opposite procedures, because contact time is the
exposure variable and you are choosing whether to include it. Read the
procedure there; the point here is that it is chemistry rather than
bureaucracy.

Three further consequences, all from EPA:

**"Since you cannot see, taste, or smell lead dissolved in water,
testing is the only sure way of telling whether there are harmful
quantities of lead in your drinking water."**

**"Remember, boiling water does not remove lead from water."** It
concentrates it, for the same reason it concentrates nitrate.

**"Use only cold water for drinking, cooking and making baby formula,"**
because hot water leaches more lead. Temperature is on the USGS list of
things that drive corrosion, and the hot water in your tank has also
been standing in contact with metal.

EPA has set the maximum contaminant level goal for lead at zero
"because lead is a toxic metal that can be harmful to human health even
at low exposure levels." The action level and the compliance machinery
around it are covered in [Testing Water](/library#testing-water); the thing to
carry from here is that there is no amount EPA describes as harmless and
the pathway runs through materials in your own building.

### What water systems do about it, and what you can

A treatment plant does not just disinfect. CDC's description of public
treatment includes a step people never hear about: "Adjusting the pH
improves taste, reduces corrosion (breakdown) of pipes, and helps
chemical disinfectants continue killing germs as the water travels
through pipes."

WHO describes the same practice in more detail. Conditioning water, it
says, normally aims at getting the bicarbonate into balance and the pH
and alkalinity to suitable values. For naturally soft water the usual
means are to push the alkalinity up, to dose a corrosion inhibitor such
as a phosphate, or both.

So a utility deliberately manages the hardness, pH and alkalinity of the
water it sends you in order to protect pipes it does not own. That is
what corrosion control is, and it is why a change in a utility's
treatment chemistry can change lead levels in houses where nothing was
touched.

A private well owner has the same problem and no chemist. The two
household equivalents follow WHO's two routes: raise the alkalinity, or
add an inhibitor. Raising alkalinity is what an acid neutralising filter
does, by running the water through carbonate media, and WHO describes
the simplest version of exactly that trick: marble chips, which are
calcium carbonate, dropped into a rainwater storage tank, where WHO says
they both add calcium to the diet and help prevent corrosion. Phosphate
feed systems are the inhibitor route. Either way, what is being treated
is not a contaminant but the water's appetite.

One last chemistry consequence that surprises people. WHO notes that
desalination and reverse osmosis strip the mineral content out and drive
the corrosivity up, and that the resulting water is so aggressive it has
to be stabilised before it can be put into a distribution system. Water
you have purified
almost completely is water with maximum room to dissolve something, so a
whole-house reverse osmosis system feeding metal pipe can create the
very problem it was bought to solve. This does not apply to a
countertop unit feeding a jug.

## What a treatment actually removes

Now the payoff. Every household treatment works on one physical
principle, that principle acts on one class of thing, and everything
outside that class passes through untouched. Learn the principles and
you can reason about a product nobody has reviewed.

CDC's framing before any of it: "Test your tap water to find out if it
has any harmful chemicals or germs," because "Different systems remove
different germs or chemicals," so "Check your treatment system's label
to make sure the system removes the chemicals or germs you are concerned
about." Buying treatment before testing is buying a solution to an
unnamed problem.

### Sediment filtration

**Principle:** a sieve. Holes smaller than the thing.

**Removes:** suspended particles larger than the pore size. Silt, rust
flakes, sand, and, as the pore size drops, organisms.
[Making Water Safe to Drink](/library#making-water-safe-to-drink) has CDC's
full ladder of pore sizes against parasites, bacteria and viruses.

**Does nothing for:** anything dissolved. Not hardness, not nitrate, not
lead, not sodium, not arsenic, not any chemical. It also does nothing
for organisms smaller than its pores, which for an ordinary cartridge
means viruses.

**The job it does that nobody credits:** it protects everything
downstream. Turbidity shields organisms from disinfection, fouls carbon,
blinds membranes and blocks ultraviolet light. A sediment stage is
usually the reason the expensive stage works.

### Activated carbon

**Principle:** adsorption. Carbon is processed to an enormous internal
surface area, and molecules that are not very comfortable dissolved in
water stick to that surface as they pass. `compounds.csv` lists it as
`activated_charcoal`, described there as "Highly porous carbon; water
filtration; poison antidote; odor removal", which is a fair summary of
what the material is for.

**Removes:** CDC's Yellow Book says granular activated carbon adsorbs
"organic and inorganic chemicals", naming "chlorine compounds, iodine
compounds, and most heavy metals", and describes the result as
"improving odor, taste, and safety." It is the standard answer for
chlorine taste, for many organic
chemicals and solvents, and EPA lists it as one of two point-of-entry
treatments for radon.

**Does nothing for:** microorganisms. CDC: carbon filters "trap, but do
not kill, microorganisms and they are generally not rated for microbe
removal." It also does very little for the small, highly water-loving
ions, because adsorption works on things that would rather leave the
water and those ions are perfectly happy in it. Hardness, sodium,
chloride and nitrate go straight through.

**Two cautions.** CDC notes that most home water filters, naming the
pitcher and fridge kind, "are not designed to remove germs," so a
carbon pitcher is a taste appliance. And whether a given cartridge
removes a given metal is a certification question, not a material
question: CDC's guidance is to look for the NSF standard on the label,
and for lead specifically
[Making Water Safe to Drink](/library#making-water-safe-to-drink) carries
CDC's recommendation of NSF/ANSI 53. "Carbon adsorbs most heavy metals"
is true of the material and not a promise about the product in your
hand.

**And the failure mode.** Adsorption capacity runs out. A saturated
cartridge does not gently stop working; it is a warm, wet surface loaded
with organic matter, which is a good place for bacteria. CDC notes that
whole-home filters that remove disinfectants can allow "more germs may
grow in your plumbing." [Testing Water](/library#testing-water) puts it well:
a filter past its service life is not neutral, it is a reservoir.

### Ion exchange

**Principle:** trading one ion for another on a resin bed. Not removal,
substitution.

**Removes:** whichever ions the resin is chosen to prefer. A softener
takes calcium and magnesium and gives back sodium, two sodium ions for
each divalent ion in WHO's description. Anion resins do the same trick
for nitrate, arsenic and sulfate. EPA names ion exchange as one of two
effective point-of-use systems for radionuclides. CDC adds that
softeners may also remove iron, manganese and some other metals.

**Does nothing for:** anything uncharged, which includes most solvents,
dissolved gases and organic chemicals, and anything alive. CDC states it
directly: water softeners do not remove parasites, bacteria or viruses.

**And the thing it adds.** Every ion exchange adds its counter-ion to
your water. A softener adds sodium; the hydrogen sulfide ion exchange
Penn State describes adds chloride. Nothing is free.

### Reverse osmosis

**Principle:** pressure forces water through a dense membrane that most
dissolved ions cannot cross. This is not sieving in the ordinary sense,
which is why it needs a pump: the membrane admits water and holds ions
back by chemistry rather than by having holes of a certain size. It is
the one process on this list other than distillation that genuinely
separates water from dissolved substances at household scale.

**Removes:** CDC lists parasites, bacteria and viruses, plus "some types
of chemicals from water, including lead, copper, chromium, chloride, and
sodium (salt)," and says it may reduce "arsenic, fluoride, radium,
sulfate, calcium, magnesium, potassium, nitrate, and phosphorous." That
list covers most of the hard cases in this guide.

**Does nothing for, or does badly with:** it is not complete for every
chemical, note CDC's careful "some types" and "may also reduce." It also
does not stand alone. The membrane is fouled by turbidity, iron and
hardness, so a real system has sediment and carbon stages in front of
it, and their maintenance is what keeps it working.

**Two costs people do not expect.** It discards a substantial share of
the feed water as concentrate. And, as WHO notes, it produces water with
almost nothing in it, which is aggressive toward metal plumbing and also
removes the calcium and magnesium that water contributes to the diet.

### Distillation

**Principle:** boil the water away from everything that does not boil,
and condense the steam. Chemically the cleanest separation available at
home.

**Removes:** CDC lists parasites, bacteria, viruses, and a long list of
chemicals: "arsenic, barium, cadmium, chromium, lead, nitrate, sodium,
sulfate, calcium, magnesium, and many organic (carbon-containing)
chemicals." Everything that is a solid when dry stays in the pot.

Salt is on that list, which matters if you are anywhere near a coast.
Reverse osmosis will desalinate too, and industrially it is the usual
way, but it needs a membrane, a pump and pressure.
[Where Water Comes From](/library#where-water-comes-from) treats distillation
as the route from seawater to drinking water for the situation that
guide is about, which is a person with a heat source and no equipment.
Collect the steam and leave the salt in the pot; there is no version of
this you can improvise with a filter.

**Does nothing for:** the thing nobody expects. CDC: distillation does
not remove "some volatile organic compounds, volatile solvents, and
certain pesticides." Volatile means they boil too. A solvent with a
boiling point near or below water's rides along in the steam and
condenses with it, so distillation can hand you a product that is clean
of everything except the class most associated with an industrial spill.
`compounds.csv` makes the risk concrete: `trichloroethylene`, an
industrial degreaser and probable carcinogen, is listed as boiling at
360 K, against 373.15 K for the `water` record. The solvent leaves the
pot before the water does.

**Cost:** CDC notes distillation "generally uses more energy and takes
longer than other water treatment systems." It is the fallback for a
problem nothing else solves, not a daily supply.

### Ultraviolet light

**Principle:** damage the organisms with light. No chemistry at all.

**Removes:** CDC's Yellow Book says ultraviolet radiation "kills
bacteria, viruses, and both Giardia and Cryptosporidium oocysts in
water," and notes "Efficacy depends on dose and exposure time." That
coverage is remarkable, because it includes both of the parasites that
defeat chlorine and the viruses that defeat ordinary microfilters.

**Does nothing for:** anything that is not alive. Not one dissolved
chemical, not hardness, not nitrate, not lead, not arsenic, not taste.
CDC's home treatment guidance lists ultraviolet systems as removing
parasites, bacteria and viruses and not removing chemicals. The water
leaving a UV unit has exactly the chemistry it had going in.

**And it is blocked by cloudiness.** CDC: "Because suspended particles
can shield microorganisms from UVR, UV irradiation units have limited
effectiveness in disinfecting water with high levels of suspended solids
and turbidity." This is the turbidity point from earlier in its purest
form. A UV lamp cannot disinfect what it cannot illuminate, which is why
these systems are always sold with a pre-filter and why CDC's listing
specifies UV "with pre-filtration."

**A structural limitation as well.** UV treats water at one point as it
passes the lamp and leaves nothing behind. Anything that gets in
downstream of the lamp is untouched, which is the opposite of chlorine,
whose whole virtue is that a residual travels with the water.

### Chemical disinfection

Doses, contact times and which chemical handles which organism are in
[Making Water Safe to Drink](/library#making-water-safe-to-drink), including
the gap where chlorine and iodine fail against Cryptosporidium. Two
chemistry points that guide does not make:

**Disinfectant is consumed, not just applied.** Chlorine reacts with
whatever is available: organic matter, iron, manganese, hydrogen
sulfide. All of that consumes dose before any of it reaches a germ.
Doubling the dose for cloudy water is not superstition; it is paying the
water's bill first.

**Disinfection makes its own byproducts.** Chlorine reacting with
organic matter forms compounds that EPA regulates as a category of their
own, disinfection byproducts, alongside microorganisms and inorganic
chemicals. This is not a reason to skip disinfection, which prevents a
far larger and far more immediate harm. It is a reason the sensible
order is to remove the organic matter first and then disinfect, rather
than chlorinating dirty water harder.

### The summary table

| Method | Works by | Removes | Does nothing for |
|---|---|---|---|
| Sediment filter | Sieving | Suspended particles above its pore size; organisms once pores are small enough | Everything dissolved; organisms smaller than its pores |
| Activated carbon | Adsorption | Chlorine and iodine compounds, many organic chemicals, most heavy metals, taste and odour, radon | Microbes; hardness; sodium; chloride; nitrate |
| Ion exchange | Swapping ions | The ions the resin prefers: hardness, or nitrate, arsenic, radionuclides | Uncharged molecules; dissolved gases; anything alive. Adds a counter-ion |
| Reverse osmosis | Membrane under pressure | Most dissolved ions, salt, many metals, and germs | Not complete for every chemical; fouls without pre-treatment; leaves aggressive water |
| Distillation | Boiling and condensing | Salts, metals, nitrate, most germs, many organics | Volatile solvents and some pesticides, which travel in the steam |
| Ultraviolet | Light damages organisms | Bacteria, viruses, Giardia, Cryptosporidium | All chemistry, with no exception; blocked by turbidity; no residual |
| Chemical disinfection | Oxidising chemistry kills organisms | Most germs, with known gaps | All chemistry problems. Consumed by organic matter. Forms byproducts |

Two lines from that table are worth saying out loud, because they are
the two mistakes people actually make:

**Ultraviolet does nothing whatsoever about chemistry.** A UV system on
a well with nitrate or arsenic produces disinfected water that is
chemically unchanged, and therefore exactly as dangerous as it was.

**Carbon does nothing whatsoever about microbes.** A carbon pitcher on
water with a coliform positive produces better-tasting water with the
same organisms in it.

**And no single method handles everything.** Every row of that table has
an entry in the last column. Any real system that covers a broad problem
is several of these in series, each covering the one before it: sediment
for the particles, carbon for the chemicals it adsorbs, membrane or
exchange for the ions, ultraviolet or chlorine for the organisms.

### The hazard that is not on the table

**A treatment system creates confidence about contaminants it does not
touch.** This is the real risk and it deserves its own sentence.

The failure is not that the equipment breaks. It is that somebody buys a
softener for the scale, notices the water is better, and stops thinking
about the well. Or installs ultraviolet after a coliform positive and
never tests for nitrate. Or runs everything through a carbon pitcher and
treats "filtered" as a synonym for "safe." In every case the equipment
worked exactly as designed, and the person is now less likely to test
than they were before they bought it, because the problem feels handled.

[Testing Water](/library#testing-water) states the correct order plainly: find
the cause before you shop for a filter, because the path that let one
thing in is still open and the system only removes what it was sized
for. The chemistry in this guide is the reason that ordering is right
rather than cautious.

### How to judge a product nobody has reviewed

Three questions, in order.

**1. What physical principle does it use?** Sieve, adsorb, exchange,
membrane, boil, or irradiate. Every product on the shelf is one or more
of those six. If the packaging talks about results and never says which
one, treat the omission as information.

**2. What class does that principle act on?** Use the table. A principle
cannot act outside its class no matter whose brand is on it.

**3. What is it certified to remove, by whom, and does that list include
your actual problem?** CDC's guidance is to look for an NSF
certification on the label and names which standard covers what:
standard 42 for taste and odour, 53 for cyst reduction, 58 for reverse
osmosis and 62 for distillation. A certification is a claim somebody
tested against a written standard. A word like "advanced," "premium" or
"multi-stage" is not.

One word on a label is better than it looks, though, and it is worth
knowing which: **"purifier" is a defined term in the United States and
"filter" is not.**
[Making Water Safe to Drink](/library#making-water-safe-to-drink) has the
detail, including the removal percentages a product must demonstrate to
use it. That is the one piece of packaging vocabulary that carries a
standard behind it rather than an adjective.

## Reading your own water as a system

The payoff of everything above is that the numbers on a laboratory sheet
stop being independent. They cluster, because they have common causes.
Three clusters cover most wells.

**Hard, alkaline, scaling water.** High hardness, high alkalinity, pH
comfortably above 7, moderate to high TDS. This is water that has spent
time in carbonate rock. Expect scale in the kettle and the heater, soap
that will not lather, and no meaningful lead risk from corrosion,
because the water is not aggressive. The problem is equipment life, and
softening the hot line is the proportionate answer.

**Soft, acidic, aggressive water.** Low hardness, low alkalinity, pH
below 7, low TDS. This is water off rock with little carbonate in it, or
off shallow sandy ground in a rainy place, and USGS names the Northwest
as one of the regions where it is common. Nothing here is a
contaminant, and that is the trap: the sheet looks clean. But this is
the water that dissolves
plumbing, so lead and copper are the questions, the first-draw sample is
the one that matters, and acid neutralisation is the treatment that
addresses the cause rather than the symptom.

**Reducing groundwater.** Iron, manganese, a rotten-egg smell, clear
water that stains after standing, and little or no dissolved oxygen.
These travel together because they have one cause: water that has been
underground long enough, in the absence of oxygen, for iron and
manganese to stay in their soluble forms and for sulfur-reducing
bacteria to work. Penn State notes dissolved iron is favoured at pH
below 7. The treatment logic is uniform across all of it: add oxygen or
another oxidant, convert the dissolved metals to particles, and filter.

Those three groupings are a synthesis of the sourced mechanisms above,
not a published classification from any agency. They are a way to read a
sheet, not a diagnosis.

Two cross-cutting readings worth knowing:

**Chloride and conductivity rising together on a coastal well** point at
seawater intrusion, which [Testing Water](/library#testing-water) treats in
detail and [Where Water Goes](/library#where-water-goes) explains is close to
irreversible. Watch the trend rather than the threshold.

**Any number moving against last year's sheet** matters more than its
absolute value, because it means something upstream changed. Be clear
about what is sourced here and what is not: the trigger CDC actually
names for testing outside the annual cycle is a change you can *notice*,
in taste, colour or smell, and
[Testing Water](/library#testing-water) reprints CDC's full list. Treating a
moved laboratory number the same way is this guide's extension of that
logic, not CDC's instruction.

## What can go wrong

- **You filtered water to remove something dissolved.** A filter is a
  sieve. It cannot catch a dissolved ion at any pore size. If the
  problem is hardness, nitrate, sodium, arsenic or lead, the answer is
  adsorption, ion exchange, a membrane or distillation, never a sieve.
- **You treated pH as a linear scale.** One unit is a factor of ten. pH
  6.5 and pH 8.5 are both acceptable and are a hundred times apart.
- **You confused pH with alkalinity.** pH is how acidic the water is
  now; alkalinity is how hard it is to change. A supply can read a
  perfectly normal pH and have no buffering at all, which is a supply
  whose pH will move the first time anything happens to it.
- **You assumed soft water is better water.** Soft, unstabilised water
  corrodes metal and can put lead, copper, cadmium and zinc into your
  glass. The two extremes cost different things.
- **You bought a softener to fix a health problem.** It exchanges
  calcium and magnesium for sodium. It is an appliance for scale and
  laundry and will not necessarily remove any serious contaminant.
- **You routed softener regeneration waste onto soil.** That brine is
  the highest-sodium, lowest-calcium water the house produces, which is
  the worst possible combination for soil structure, and
  [Greywater](/library#greywater) records that greywater rules exclude it by
  name. Softened water itself is a milder version of the same trade,
  more sodium and less calcium, so read that guide before irrigating
  with it either.
- **You spent money on an aesthetic exceedance.** Orange staining is
  iron; iron at 0.4 mg/L will not hurt anybody, and EPA does not enforce
  the number you exceeded.
- **You smelled rotten eggs and went to look.** Hydrogen sulfide kills
  in confined spaces, the odour is not a dose meter, and continued
  exposure can deaden your sense of smell. Never enter a septic tank,
  cistern, sump or well pit.
- **You replaced a whole well system because the hot water smelled.**
  If only the hot tap smells, suspect the water heater's magnesium
  anode rod first.
- **You boiled water that had nitrate or lead in it.** Boiling drives
  off water as steam, and nitrate and lead both stay in the pot, so what
  is left is more concentrated than what you started with. (Boiling does
  take the carbonate share of hardness out of solution, as the kettle
  crust shows, but that is a scale-forming mineral dropping out. It is
  not a general power to remove chemicals, and it is the exception
  rather than the rule.)
- **You installed ultraviolet and stopped thinking about chemistry.** UV
  is excellent against organisms and does absolutely nothing to a
  dissolved chemical.
- **You ran a carbon pitcher and called the water safe.** Carbon traps
  but does not kill. Most pitcher and fridge filters are not designed to
  remove germs at all.
- **You distilled water to get rid of a solvent smell.** Volatile
  solvents boil with the water and condense with it. Distillation is the
  wrong tool for exactly the contaminants a fuel or solvent spill
  delivers.
- **You left a filter cartridge in past its life.** A saturated
  cartridge is not a neutral object, it is a wet organic-rich surface
  that grows things.
- **You tested at a tap with a softener or a filter on it.** You
  measured the appliance.
- **You judged your treatment by whether the water tastes better.**
  Taste responds to chlorine, iron, hardness and dissolved gases. It
  does not respond to nitrate, arsenic, lead or radon at any
  concentration that matters.

## How you know it worked

- You can say, for any substance you are worried about, whether it is
  suspended or dissolved, and therefore whether a filter can possibly
  touch it.
- You can explain why a pH of 6 is ten times more acidic than a pH of 7,
  and why that does not tell you how much acid it would take to get
  there.
- You know whether your water's pH is backed by alkalinity or not, and
  what that predicts about how stable it is.
- You know roughly what your hardness is, whether that is a scale
  problem worth money, and that it is not a health problem.
- If you have a softener, you can say what it puts into the water, where
  its regeneration waste discharges to, and why that brine is the one
  thing in the house that must not reach soil.
- You can say what TDS, conductivity and turbidity each measure, and
  which of the three would move if your well started drawing salt.
- You know why cold fast water holds more oxygen than warm slow water,
  and therefore why a warm August pool is a different habitat from the
  same creek in March.
- If you have ever smelled rotten eggs on the property, you know whether
  it is only the hot tap, and you know not to put your head into any
  enclosed space that smells of it.
- If there is an infant or a pregnancy in the house and you are on a
  well, you have a nitrate result measured as nitrogen, from within the
  last year.
- You know whether your water is the aggressive kind, and if it is, you
  have a first-draw lead result rather than a flushed one.
- For every piece of treatment equipment in your house, you can name the
  physical principle it works by, the class of thing it acts on, and at
  least one class of thing it does nothing about.

When those are true, the four water guides stop being lists of
thresholds. A number on a sheet becomes a statement about rock,
contact time and temperature, and a product on a shelf becomes a claim
you can check. That is the whole aim: not to memorise more limits, but
to need fewer of them.

## Sources

Grouped by what kind of authority each one is. United States government
publications are listed first because they are public domain and can be
redistributed with this guide; the rest cannot, so every fact taken from
them is restated here in our own words.

### United States government (public domain)

- United States Geological Survey, Water Science School. Water, the
  Universal Solvent (water as the universal solvent, the polar
  arrangement of oxygen and hydrogen, the positive hydrogen side and
  negative oxygen side, the mechanism by which water pulls sodium and
  chloride apart, and that water cannot dissolve everything).
  https://www.usgs.gov/special-topics/water-science-school/science/water-universal-solvent
- United States Geological Survey, Water Science School. Water Q&A: Why
  is water the "universal solvent"? (the same polarity and salt
  statements in a second wording, and that water carries chemicals,
  minerals and nutrients wherever it goes).
  https://www.usgs.gov/special-topic/water-science-school/science/water-qa-why-water-universal-solvent
- United States Geological Survey, Water Science School. pH and Water
  (the definition of pH, the 0 to 14 scale with 7 neutral, that pH is
  logarithmic and each number is a tenfold change, the pH 5 versus pH 6
  example, normal rainfall at about pH 5.6 because of atmospheric carbon
  dioxide, and coal mine water at pH 2).
  https://www.usgs.gov/special-topics/water-science-school/science/ph-and-water
- United States Geological Survey, Water Science School. Alkalinity and
  Water (alkalinity as buffering capacity, that it is a property rather
  than a chemical, bicarbonates carbonates and hydroxides as its source,
  measurement by titration with acid, the limestone versus granite
  contrast, and that high alkalinity water changes less when acid is
  added). https://www.usgs.gov/water-science-school/science/alkalinity-and-water
- United States Geological Survey, Water Science School. Hardness of
  Water (the definition of hardness as dissolved calcium and magnesium,
  water dissolving minerals as it moves through soil and rock, the soft
  to very hard bands in mg/L as calcium carbonate, that hardness is not
  a health concern but can be a nuisance, mineral buildup in plumbing
  fixtures and water heaters, calcium carbonate scale forming when hard
  water is heated, and soap reacting with calcium to form soap scum).
  https://www.usgs.gov/special-topics/water-science-school/science/hardness-water
- United States Geological Survey, Water Science School. Conductivity
  (Electrical Conductance) and Water (pure water as an excellent
  insulator, salts dissolving into cations and anions, that even a small
  amount of ions makes water able to conduct electricity, and sodium
  chloride as the most familiar dissolved substance).
  https://www.usgs.gov/special-topics/water-science-school/science/conductivity-electrical-conductance-and-water
- United States Geological Survey, Water Science School. Dissolved
  Oxygen and Water (what dissolved oxygen is, oxygen entering from the
  atmosphere and from groundwater discharge, cold water holding more
  than warm, rapidly moving water holding more than stagnant, more than
  8 mg/L typical in summer surface waters, hypoxia below 2 mg/L, and
  eutrophic conditions from excess organic material).
  https://www.usgs.gov/special-topics/water-science-school/science/dissolved-oxygen-and-water
- United States Geological Survey, Water Science School. Turbidity and
  Water (the definition as relative clarity measured by light
  scattering, the materials that cause it, nephelometric turbidity
  units, that turbidity is not a direct indicator of health risk, the
  relationship between turbidity removal and protozoa removal, and that
  particles shelter microbes from disinfectants).
  https://www.usgs.gov/special-topics/water-science-school/science/turbidity-and-water
- United States Geological Survey, Water Science School. Groundwater
  Quality (water as an excellent solvent with a lot of opportunity to
  dissolve substances underground, iron and manganese being dissolved as
  groundwater flows through sediments, and groundwater often having more
  dissolved substances than surface water).
  https://www.usgs.gov/special-topics/water-science-school/science/groundwater-quality
- United States Geological Survey. Chloride, Salinity, and Dissolved
  Solids (total dissolved solids as the sum of all substances dissolved
  in water, the list of constituents that make up most dissolved solids,
  the problems associated with elevated dissolved solids, salinity as
  another term for dissolved solids content, and specific conductance as
  the surrogate used to monitor dissolved solids in real time).
  https://www.usgs.gov/mission-areas/water-resources/science/chloride-salinity-and-dissolved-solids
- United States Geological Survey. Karst Aquifers (karst as terrain
  created from the dissolution of soluble rocks, principally limestone
  and dolomite, and about 40 percent of groundwater used for drinking in
  the United States coming from karst aquifers).
  https://www.usgs.gov/mission-areas/water-resources/science/karst-aquifers
- United States Geological Survey. All About Corrosivity (corrosivity as
  how aggressive water is at corroding pipes and fixtures; pH, calcium
  concentration, hardness, alkalinity, dissolved solids and temperature
  as the properties that decide it; corrosive water dissolving lead and
  other metals from household plumbing; the Langelier Saturation Index
  and the Potential to Promote Galvanic Corrosion; the assessment of
  more than 20,000 wells across 25 states; and that naturally corrosive
  water is not dangerous to consume by itself but causes health problems
  by reacting with plumbing).
  https://www.usgs.gov/mission-areas/water-resources/science/all-about-corrosivity
- United States Geological Survey. New Study Shows High Potential for
  Groundwater to be Corrosive in Half of U.S. States (the 25 states
  figure, the Northeast, Southeast and Northwest as the regions with the
  largest percentage of wells with potentially corrosive groundwater,
  about 44 million people drinking from private wells, and private
  supplies being the sole responsibility of the homeowner).
  https://www.usgs.gov/news/new-study-shows-high-potential-groundwater-be-corrosive-half-us-states-0
- United States Geological Survey. National Field Manual for the
  Collection of Water-Quality Data, Chapter A6 section 6.2, Dissolved
  Oxygen, version 3.0 (table 6.2-6, solubility of oxygen in freshwater
  at various temperatures and pressures, from which the 760 millimetre
  of mercury column values at 0, 5, 10, 15, 20, 25 and 30 degrees C are
  taken; and the sources and sinks of dissolved oxygen including
  respiration, aerobic decomposition and ammonia nitrification).
  https://pubs.usgs.gov/twri/twri9a6/twri9a62/twri9a6_6.2_ver3.pdf
- Environmental Protection Agency. National Primary Drinking Water
  Regulations (the nitrate and nitrite MCLGs and MCLs measured as
  nitrogen with their health effects and sources; the definitions of
  MCL, MCLG and treatment technique; the lead MCLG of zero; and the
  turbidity entry describing higher turbidity levels as often associated
  with higher levels of disease-causing microorganisms).
  https://www.epa.gov/ground-water-and-drinking-water/national-primary-drinking-water-regulations
- Environmental Protection Agency. Secondary Drinking Water Standards:
  Guidance for Nuisance Chemicals (the secondary maximum contaminant
  levels and noticeable effects for iron, manganese, total dissolved
  solids and pH; the rotten-egg odour entry; and the statements that EPA
  does not enforce these standards and that the contaminants are not
  considered to present a risk to human health at the SMCL).
  https://www.epa.gov/sdwa/secondary-drinking-water-standards-guidance-nuisance-chemicals
- Environmental Protection Agency. Basic Information about Lead in
  Drinking Water (lead entering water when plumbing materials corrode,
  especially where water has high acidity or low mineral content; the
  definition of corrosion; lead pipes, faucets, fixtures, brass and
  solder as sources; lead service lines as typically the most
  significant source; that lead cannot be seen, tasted or smelled;
  that boiling does not remove lead; cold water only for drinking,
  cooking and formula; that the longer water sits in pipes the more lead
  it may contain; and the maximum contaminant level goal of zero).
  https://www.epa.gov/ground-water-and-drinking-water/basic-information-about-lead-drinking-water
- Environmental Protection Agency. Potential Well Water Contaminants and
  Their Impacts (nitrates being converted to nitrites in the body,
  methemoglobinemia and blue baby syndrome, the reduction in the blood's
  ability to carry oxygen, the acute onset over days, the symptoms of
  shortness of breath and blueness of the skin, and infants below six
  months; plus the heavy metal sources and health effects).
  https://www.epa.gov/privatewells/potential-well-water-contaminants-and-their-impacts
- Environmental Protection Agency. Natural Radionuclides in Private
  Wells (radium breaking down to form radon, all of these elements
  dissolving in water and accumulating in wells, the recommendation to
  test well water every three years for radionuclides, and ion exchange
  and reverse osmosis as effective point-of-use systems).
  https://www.epa.gov/radtown/natural-radionuclides-private-wells
- Environmental Protection Agency. Radon in Drinking Water: Questions
  and Answers (radon dissolving and accumulating in groundwater; radon
  not being a concern in surface water because it is released before
  reaching the tap; the estimate of about 168 cancer deaths a year with
  89 percent from lung cancer caused by breathing radon released to
  indoor air from water and 11 percent from stomach cancer; radon
  escaping to the air during showering, dishwashing and cooking; and
  granular activated carbon filters and aeration devices as the two
  point-of-entry treatments). **This page is in EPA's web archive rather
  than its current site**, and the associated regulation was a proposal;
  it is cited here for the exposure mechanism and the relative risk
  split, not as current regulation.
  https://archive.epa.gov/water/archive/web/html/qa1.html
- Centers for Disease Control and Prevention. About Home Water Treatment
  Systems (the instruction to test tap water first, that different
  systems remove different germs or chemicals, and the removal and
  non-removal lists for microfiltration, ultrafiltration,
  nanofiltration, reverse osmosis, distillation, ultraviolet treatment
  systems and water softeners, including distillation not removing some
  volatile organic compounds, volatile solvents and certain pesticides,
  and softeners not removing parasites, bacteria or viruses).
  https://www.cdc.gov/drinking-water/about/about-home-water-treatment-systems.html
- Centers for Disease Control and Prevention. About Choosing Home Water
  Filters (the NSF/ANSI standards 42 for taste and odour, 53 for cyst
  reduction, 58 for reverse osmosis and 62 for distillation; absolute
  pore sizes for parasites and bacteria; that most home water filters
  such as pitcher or fridge filters are not designed to remove germs;
  and that whole-home filters removing disinfectants may allow more
  germs to grow in plumbing).
  https://www.cdc.gov/drinking-water/prevention/about-choosing-home-water-filters.html
- Centers for Disease Control and Prevention. How Water Treatment Works
  (coagulation, flocculation, sedimentation, filtration and
  disinfection, and that adjusting the pH improves taste, reduces
  corrosion of pipes, and helps chemical disinfectants keep working as
  water travels through pipes).
  https://www.cdc.gov/drinking-water/about/how-water-treatment-works.html
- Centers for Disease Control and Prevention. Water Disinfection for
  Travelers, CDC Yellow Book (ultraviolet radiation killing bacteria,
  viruses and both Giardia and Cryptosporidium oocysts with efficacy
  depending on dose and exposure time; suspended particles shielding
  microorganisms so that UV units have limited effectiveness in turbid
  water; granular activated carbon adsorbing organic and inorganic
  chemicals including chlorine compounds, iodine compounds and most
  heavy metals; and that carbon filters trap but do not kill
  microorganisms and are generally not rated for microbe removal).
  https://www.cdc.gov/yellow-book/hcp/preparing-international-travelers/water-disinfection-for-travelers.html
- Agency for Toxic Substances and Disease Registry. Hydrogen Sulfide,
  Toxic Substances Portal (hydrogen sulfide as a colorless, flammable,
  highly toxic gas with a characteristic rotten-egg odor; production by
  decaying organic matter and release from sewage sludge, liquid manure,
  sulfur hot springs and natural gas; inhalation as the major route of
  exposure; the odour threshold of 0.5 parts per billion; irritation of
  the nose, throat and lower respiratory tract at 50 ppm, which ATSDR
  labels a low concentration; extremely rapid unconsciousness and death
  from high
  concentrations; olfactory fatigue; that continued exposure at high
  levels may deaden the sense of smell; and that odor is not a reliable
  indicator and may not provide adequate warning of hazardous
  concentrations).
  https://wwwn.cdc.gov/TSP/ToxFAQs/ToxFAQsDetails.aspx?faqid=385&toxid=67
- Agency for Toxic Substances and Disease Registry. Hydrogen Sulfide,
  Medical Management Guidelines (that hydrogen sulfide is slightly
  heavier than air and may accumulate in enclosed, poorly ventilated and
  low-lying areas; that fatalities have occurred to rescuers entering
  the hot zone; and that rescuers should have a safety line because of
  the extremely rapid toxic action of the gas).
  https://wwwn.cdc.gov/tsp/MMG/MMGDetails.aspx?mmgid=385&toxid=67
- National Park Service, Caves and Karst. Making a Cave (precipitation
  mixing with carbon dioxide from air and decaying plants in soil to
  form carbonic acid, and that acid seeping into carbonate rocks such as
  marble, limestone and dolomite and dissolving them).
  https://www.nps.gov/subjects/caves/making-a-cave.htm
- National Park Service, Caves and Karst. Speleothems (that with the
  loss of carbon dioxide a thin film of carbonate material precipitates,
  and that where a drop lands carbon dioxide is given off and carbonate
  material is precipitated as a mound below the point of dripping).
  https://www.nps.gov/subjects/caves/speleothems.htm
- National Oceanic and Atmospheric Administration, National Ocean
  Service. Ocean Acidification (carbon dioxide absorbed by seawater
  producing a series of reactions that increase the concentration of
  hydrogen ions, making the water more acidic and making carbonate ions
  relatively less abundant, and the ocean absorbing about 30 percent of
  released carbon dioxide).
  https://oceanservice.noaa.gov/facts/acidification.html
- National Oceanic and Atmospheric Administration, National Ocean
  Service. Why is the ocean salty? (rainwater that falls on land being
  slightly acidic so that it erodes rocks, releasing ions carried to
  streams and rivers and eventually the ocean, and average seawater
  salinity of about 35 parts per thousand).
  https://oceanservice.noaa.gov/facts/whysalty.html
- Food and Drug Administration. Sodium in Your Diet (the Daily Value for
  sodium of less than 2,300 mg per day, average American intake of about
  3,400 mg per day, lower recommended limits for children under 14, and
  the association between higher-sodium diets and high blood pressure).
  https://www.fda.gov/food/nutrition-education-resources-materials/sodium-your-diet

### Not United States federal, cited as the authority and restated in our own words

- World Health Organization. Hardness in Drinking-water, background
  document for development of WHO Guidelines for Drinking-water Quality,
  WHO/HSE/WSH/10.01/10/Rev/1, 2011 (hardness as the traditional measure
  of the capacity of water to react with soap; the carbonate, temporary
  and non-carbonate, permanent distinction; the soft to very hard bands;
  calcium and magnesium from sedimentary rocks, most commonly limestone
  and chalk; other polyvalent cations including iron and manganese
  contributing; drinking water supplying 5 to 20 percent of calcium and
  magnesium intake; scale in heated water applications reducing heat
  exchanger efficiency; increased soap consumption and bathtub ring;
  unstabilised soft water corroding metal and producing cadmium, copper,
  lead and zinc in drinking water; conditioning targeting bicarbonate
  equilibrium with suitable pH and alkalinity, and phosphate corrosion
  inhibitors for naturally soft water; desalinated and reverse osmosis
  water being highly aggressive and needing stabilisation; rainwater
  being soft and usually slightly acidic; ion exchange replacing each
  divalent ion with two sodium ions and increasing sodium and chloride
  content; softening only the hot water line as an approach; the
  epidemiological picture on cardiovascular mortality being debated and
  not proving causality; eczema as a suggestion with studies under way;
  and that there are insufficient data to suggest minimum or maximum
  mineral concentrations, so no guideline values are proposed).
  https://cdn.who.int/media/docs/default-source/wash-documents/wash-chemicals/hardness-bd.pdf
- Penn State Extension. Iron and Manganese in Private Water Systems (the
  0.3 mg/L and 0.05 mg/L recommended limits; the non-enforceable EPA
  health advisory for manganese of 0.3 mg/L addressing neurological
  effects from prolonged exposure; water appearing colourless at the tap
  because the metals have not yet reacted with oxygen, with orange-brown
  or black particles forming after exposure to air; dissolved reduced
  iron being favoured at pH below 7.0; natural sources being more common
  in deeper wells with longer rock contact; and the treatment options by
  concentration range, including ion exchange below about 5 mg/L
  dissolved iron, polyphosphate sequestration below about 2 mg/L,
  oxidising filters for roughly 3 to 10 mg/L combined, and chlorination
  with filtration above about 10 mg/L).
  https://extension.psu.edu/iron-and-manganese-in-private-water-systems
- Penn State Extension. Water Softening (the ion exchange mechanism with
  resin coated in sodium; regeneration by backwashing with brine that
  carries away the captured calcium and magnesium; about 50 gallons of
  water per regeneration cycle; the addition of 7.5 milligrams of sodium
  per quart for each grain per gallon of hardness removed; one grain per
  gallon being approximately 17 mg/L; the sodium-restricted diet
  consideration; the loss of dietary calcium and magnesium; and that
  water softening will not necessarily remove any of the more serious
  drinking water contamination problems).
  https://extension.psu.edu/water-softening
- Penn State Extension. Hydrogen Sulfide (Rotten Egg Odor) in Water
  Wells (sulfur-reducing bacteria feeding on sulfur and thriving in the
  low-oxygen environment of wells and plumbing; the problem being most
  common in wells drilled into acidic bedrock such as shale and
  sandstone; the magnesium anode rod in a water heater chemically
  reducing sulfates to form hydrogen sulfide, and the hot-water-only
  smell that indicates it, with an aluminium rod as the replacement;
  detection below about 0.5 mg/L; there being no drinking water standard
  because the water becomes undrinkable long before harmful
  concentrations; corrosion of plumbing metals and yellow or black
  greasy stains from metallic sulfides; and the treatment options
  including aeration, ion exchange, which works but adds chloride,
  activated carbon below about 1.0 mg/L, oxidising
  filters, chlorination and potassium permanganate).
  https://extension.psu.edu/hydrogen-sulfide-rotten-egg-odor-in-water-wells

### Inside this project

- `data/chemistry/compounds.csv`, for the records referenced by id:
  `water`, `sodium_chloride`, `potassium_chloride`, `saltpeter`,
  `ethanol`, `acetic_acid`, `glucose`, `octane`, `kerosene`, `toluene`,
  `benzene`, `calcium_carbonate`, `gypsum`, `carbonic_acid`,
  `stearic_acid`, `radon_gas`, `activated_charcoal` and
  `trichloroethylene`. The solubility figures for calcium carbonate and
  gypsum, and the boiling points of trichloroethylene and water, are
  quoted from that file rather than from an agency page. Note that
  `stearic_acid` is one of 19 rows in that file carrying a stray extra
  comma, which shifts its later columns; the only thing quoted from it
  here is text from its description, which the raw line does contain.
- [Where Water Comes From](/library#where-water-comes-from), for the sources
  themselves, the point that clarity predicts nothing about safety, and
  distillation as the only route from seawater to drinking water.
- [Where Water Goes](/library#where-water-goes), for the watershed, the
  Silverdale soils and their glacial parent material, thermal pollution
  from impervious surfaces, and seawater intrusion.
- [Testing Water](/library#testing-water), for the annual test list, the full
  secondary standards table, the hardness bands in their
  report-reading context, the nitrate units trap, the lead action level
  and its revision, the first-draw versus flushed sampling distinction,
  the instruction not to sample at a softened tap, and the order of
  response to a bad result.
- [Making Water Safe to Drink](/library#making-water-safe-to-drink), for
  filter pore sizes against organisms, boiling, chemical disinfection
  doses and contact times, the gaps against Cryptosporidium and
  Giardia, and the NSF/ANSI 53 recommendation for lead.
- [Greywater](/library#greywater), for the sodium adsorption ratio, why sodium
  destroys soil structure, and why softener regeneration waste is
  excluded from greywater systems by name.
- [Units and Converting Them](/library#units-and-converting-them), for reading
  a scale for what it actually measures.
- `data/locales/silverdale_wa/water.json`, consulted for local water
  chemistry and found to contain none: it carries the watershed,
  surface waters and groundwater hydrology but no hardness, pH,
  alkalinity, dissolved solids, conductivity, iron, manganese or nitrate
  figures. The local inference in the corrosivity section is therefore
  unchecked against local data, and is labelled as an inference in the
  text.

### Arithmetic rather than citation

Every calculation below is a multiplication or a ratio of numbers
sourced above; none of them is a claim about anything on its own.

- pH 6.5 to 8.5 spans two logarithmic units, so a hundredfold range.
  Rain at pH 5.6 is 1.4 units below neutral, so about 25 times more
  acidic than neutral water. Coal mine water at pH 2 is five units
  below neutral, so a hundred thousand times.
- Oxygen solubility falls from 14.62 mg/L at 0 degrees C to 7.56 mg/L at
  30 degrees C, a reduction of 7.06 mg/L, which is about 48 percent of
  the starting value.
- Gypsum at 2.0 g/L against calcium carbonate at 0.013 g/L is a ratio of
  about 154, given in the text as about 150 times.
- Very hard water at 180 mg/L as calcium carbonate, divided by Penn
  State's conversion of about 17 mg/L per grain per gallon, is about
  10.6 grains per gallon. At Penn State's rate of 7.5 mg of sodium per
  quart for each grain per gallon removed, that is about 79 mg of sodium
  per quart, or about 160 mg in two quarts. Against FDA's Daily Value of
  less than 2,300 mg, 160 mg is about 7 percent.
- Manganese's aesthetic limit of 0.05 mg/L against its health advisory
  of 0.3 mg/L is a factor of six.
- ATSDR's 50 ppm, the concentration it calls low, against ATSDR's odour
  threshold of 0.5 parts per billion, which is 0.0005 ppm, is a factor
  of 100,000.
- The 25 states USGS names out of 50 is half.

### Labelled in the text as reasoning or rules of thumb, not sourced

- The phrase "like dissolves like," and the naming of the hydrogen bond.
  USGS describes water's polarity and the attraction between water
  molecules without using either term, and no page opened for this guide
  states the generalisation. The polarity, the salt mechanism and the
  attraction itself are sourced; the vocabulary and the generalisation
  are ordinary textbook material.
- The statement that a conductivity meter cannot identify which ions are
  present, and is blind to uncharged dissolved substances. This follows
  from the mechanism USGS describes, but is not stated in those words on
  any page opened here.
- The charge explanation for soap scum, that calcium carries two
  positive charges and can therefore pair with two soap molecules into
  something insoluble. The outcome is sourced twice over, by USGS on
  soap reacting with calcium to form soap scum and by WHO on hard water
  needing considerably more soap and leaving a deposit. The mechanism is
  textbook chemistry and is flagged as such in the text.
- The explanation for why activated carbon does not remove nitrate, that
  adsorption acts on substances that would rather not be dissolved while
  nitrate is a small, singly charged, highly water-loving ion. The
  conclusion rests on nitrate being absent from CDC's lists of what
  carbon adsorbs and present on CDC's lists for reverse osmosis and
  distillation; the reason is textbook chemistry and is flagged as such
  in the text.
- The thermometer-and-radiator analogy for pH and alkalinity. Ours.
- The claim that water around Silverdale is likely to be soft, low in
  alkalinity and therefore corrosive. This is an inference from three
  sourced facts, USGS naming the Northwest among the corrosive regions,
  WHO describing rainwater as soft and slightly acidic, and the local
  soil survey showing glacial outwash and till rather than carbonate
  rock. No local hardness, pH or alkalinity measurement was found, and
  the locale water file contains none.
- The three clusters in "Reading your own water as a system," which are
  a synthesis of the sourced mechanisms above rather than a published
  classification.
- The three questions for judging an unreviewed product. The NSF
  standard numbers are CDC's; the framing of the questions is ours.
- Treating a moved laboratory number as a reason to test outside the
  annual cycle. CDC's published trigger is a change the householder
  notices in taste, colour or smell; extending that to a change in a
  measured value is this guide's reasoning, and the text says so at the
  point it is used.
- The ordering and emphasis throughout, including the claim that the
  dissolved-versus-suspended distinction is the most useful idea in the
  guide.

### Could not be sourced

- A federal statement of the temporary and permanent hardness
  distinction. USGS's hardness page does not use the terms carbonate
  hardness, noncarbonate hardness, temporary or permanent. The
  distinction in this guide rests on WHO, which is not a United States
  federal work, so it is restated rather than quoted.
- A page explaining why nonpolar substances such as oil do not dissolve
  in water. NOAA's oil spill chemistry page, which looked like the right
  place, covers the composition of oil and says nothing about its
  solubility. The claim in this guide rests on the observable solubility
  values in `data/chemistry/compounds.csv` and on water's polarity as
  USGS describes it, not on a source that states the principle.
- Four pages would not load and are therefore not cited anywhere above:
  the USGS Hydrologic Atlas chapter on carbonate-rock aquifers and a
  USGS Water-Resources Investigations report on water-quality criteria
  (both HTTP 403), and a National Institutes of Health PubMed Central
  review of the health impacts of hard water (blocked behind a
  reCAPTCHA). The material they would have supplied on carbonate versus
  noncarbonate hardness came from WHO instead.
- EPA's technical recommendations on optimal corrosion control treatment
  are published only as a linked PDF behind a landing page with no
  substantive text; the corrosion control description in this guide
  comes from CDC and WHO instead.
