# Metals and Alloys: Why the Thing Broke

Find a paperclip. Straighten one bend, then bend it back. Then back
again, in the same place, as fast as you like. Count.

Somewhere between the third and the tenth bend it snaps. Three things
just happened, and between them they explain most of what metal does.

**The first bend was easy.** You pushed past the point where the metal
springs back and it stayed where you put it. That is plastic
deformation, and it is the whole reason metal is useful.

**The second bend, at the same spot, was harder.** You did not heat it,
alloy it or hammer it. You bent it, and bending it made it stronger.
That is work hardening.

**Then it broke, at a load nowhere near what it takes to pull a
paperclip in half.** You never came close to tearing it. You cycled it,
and cycling it killed it. That is fatigue, and it is the commonest way
real metal parts fail in real life.

One paperclip, twenty seconds, and you have done the experiment that
underlies work hardening, annealing, tempering, and why a bolt that held
for six years let go on an ordinary Tuesday.

## What this guide will and will not do

It will give you one mechanism that most metal behaviour falls out of,
then use it to answer the questions people actually have. Why did the
bolt shear. Why did the bracket eat what it was bolted to. Why is the
cheap knife either soft or brittle and never both. Why did the weld
crack beside the weld instead of in it. Why does the same stainless rust
at the marina and not in the kitchen. Why does a ladder rated for 300
pounds fail under less than 300 pounds.

It will not teach you to heat treat steel. Reading cannot do that, and
the reason is worth saying plainly: **the right temperature, soak time
and quenching liquid are different for every alloy, and a recipe that
makes a good tool from one steel will crack or ruin another.** The
Army's welding manual gives the general rule in one sentence: the
presence of alloying elements alters the rate of transformation on
cooling, each element shows individuality in its effect, and so alloy
steels are heat treated to meet specific requirements. If you do not
know which steel you have, you do not know its recipe, and this guide
will not invent one.

Every number here comes from a federal publication opened and read for
this guide, all listed at the bottom with what each gave. Where
something is convention rather than a published measurement, it says so.

## The one idea: a crystal with mistakes in it

A metal is a stack of atoms in a repeating lattice, and the useful thing
about metals is that the pattern is never perfect.

The Department of Energy's materials handbook describes the common
arrangements: body-centred cubic, face-centred cubic and hexagonal
close-packed. Iron uses more than one at different temperatures, which
is the reason steel can be heat treated at all.

A solid lump is not one crystal. It is a mass of small crystals called
**grains**, each with its lattice pointing a different way, meeting at
**grain boundaries**.

Now the mistakes. The handbook divides them into point defects, single
atoms missing or misplaced, and **line defects, called dislocations**,
which are many atoms long. The commonest kind is an extra half-row of
atoms wedged into the lattice. A dislocation cannot simply stop in the
middle of a crystal: it must run out to an edge, meet another
dislocation, or close back on itself.

**Here is the payoff.** When you bend metal, the layers of atoms do not
all slide at once. That would take an enormous force. Instead the
dislocation moves: stress is applied, the extra half-row shifts one
atomic spacing, then another, and travels through the crystal until it
reaches an edge or is stopped by another dislocation. The handbook calls
this slip.

Think of moving a heavy rug by dragging the whole thing versus by
pushing a ripple along it. The ripple is the dislocation. Metal is soft
and workable because it has ripples. **Everything that makes a metal
stronger works by making the ripples harder to move.**

Four consequences follow, and all four are visible.

**Work hardening.** Deformation multiplies and tangles dislocations. The
handbook describes cold work as plastic deformation where the strain
hardening is not relieved, forming high dislocation density regions that
develop into networks. Ripples running into each other jam. The metal
gets stronger and, in the same breath, **less ductile**: cold working
will decrease ductility. That is your paperclip.

**Alloying.** Foreign atoms obstruct the ripples. The handbook says so
almost word for word about one element: copper does not form a carbide
but increases hardness by retarding dislocation movement. Different
elements obstruct differently. Chromium forms a carbide that hardens the
metal and can also occupy lattice sites, raising hardness without
hurting ductility; nickel below about 5 percent raises toughness and
ductility without raising hardness, precisely because it forms no
carbides.

**Annealing.** Heat it enough and the atoms return toward their
equilibrium positions, the tangles unwind, and new grains grow. The
handbook lists the purposes: to soften the steel and improve ductility,
to relieve internal stresses from heat treatment, welding or machining,
and to refine the grain structure. Anneal your paperclip and you could
start bending again.

**Grain size.** Boundaries stop ripples too, so more boundaries means a
stronger metal. The operational rule, from the handbook: generally, the
faster a metal is cooled, the smaller the grain sizes, and this will
make the metal harder. That one sentence is the basis of the entire heat
treating trade.

Hold on to the ripple picture. Everything below follows from it.

## Strength is five different numbers

This is where most practical mistakes start. "Stronger" is not one
property. It is at least five, they trade against each other, and the
one people most often want is the one they least often ask for.

### Stiffness, which barely changes at all

**Stiffness is resistance to bending and stretching within the elastic
range**, where the metal springs back. It is Young's modulus, defined by
the DOE handbook as the ratio of stress to strain below the proportional
limit, or the slope of the straight part of a stress-strain curve.

Now the number that surprises people. The handbook's own table of common
structural materials:

| Material | Young's modulus | Yield strength | Ultimate strength |
|---|---|---|---|
| Carbon steel | 30 x 10^6 psi | 30,000 to 40,000 psi | 55,000 to 65,000 psi |
| Stainless steel | 29 x 10^6 psi | 40,000 to 50,000 psi | 78,000 to 100,000 psi |
| Aluminium | 10 x 10^6 psi | 35,000 to 45,000 psi | 54,000 to 65,000 psi |

Read the first column against the third. The stainless is up to half
again as strong at the top of its range, and its stiffness is slightly
**lower**. In modern units 30 x 10^6 psi is 207 GPa and 29 x 10^6 psi is
200 GPa, a difference of about 3 percent against a strength difference
of roughly 50 percent.

The same shows up inside one alloy. NASA's data handbook for aluminium
6061 gives the annealed condition an ultimate of 18.0 ksi and the T6
condition 45.0 ksi, a factor of 2.5, and lists the tensile modulus once,
at 10.0 x 10^3 ksi, because heat treating does not change it. The 7075
handbook does the same: 33.0 ksi annealed against 83.0 ksi at T6, one
modulus of 10.4 x 10^3 ksi.

**What this means.** Stiffness is a property of the bonds between atoms.
Heat treatment, cold work and modest alloying rearrange dislocations
rather than change bonds. So:

- **A shelf that sags cannot be fixed by buying stronger steel.** Same
  shape, same span, same load, same sag. To stop it sagging, change the
  shape, shorten the span, or add material where the bending is.
- **Aluminium of the same shape deflects about three times as much as
  steel**, since 30 million psi against 10 million psi is a ratio of
  exactly 3. This is not fixed by picking a stronger aluminium, because
  6061 and 7075 have nearly the same modulus and wildly different
  strengths. It is why an aluminium ladder feels springy.

**What it does not mean.** Steels differ enormously in when they stop
springing back, in how much abuse they take, and in how they corrode.
Stiffness is the one property that stays put.

### Yield: where it stops springing back

**The yield strength is the stress at which the metal takes a permanent
set.** Below it the part returns to shape; above it, it does not.

For most metals there is no sharp knee, so yield is defined by
convention: draw a line parallel to the elastic part of the curve,
offset by a chosen permanent strain, and take the stress where it
crosses. The DOE handbook insists the offset be stated, giving as its
example "Yield Strength (at 0.2 percent offset) = 51,200 psi". **A yield
figure that does not say what offset it used is incompletely
specified.**

Yield is the number that matters for anything that must keep its shape.

### Ultimate tensile strength: where it comes apart

**The highest stress reached before it breaks.** The Army manual defines
it as the ability to resist being pulled apart by opposing forces acting
in a straight line.

It is the number most often printed, because it is the biggest. It is
usually the least useful, because a bent bracket has already failed
though it has not parted. Notice from the table how much room sits
between the two for carbon steel: yield around 30,000 to 40,000 psi
against breaking around 55,000 to 65,000, which is a comfortable margin
of warning. A hardened tool steel has almost none, which is another way
of saying that when it goes, it goes without bending first.

### Hardness: resistance to being dented

**Resistance to penetration and wear by another material**, per the Army
manual. Measured by pressing a standard indenter in with a standard
force: Brinell, Rockwell, Vickers.

It correlates usefully with strength. The DOE handbook gives a
conversion worth knowing: for quenched and tempered steel, the tensile
strength in psi is about 500 times the Brinell number, provided the
strength is not over 200,000 psi. Take its own example of 352 Brinell:
times 500 is 176,000 psi, which is 1,213 MPa. **That is a rule of thumb
with a stated ceiling**, for quenched and tempered steel, not for
aluminium, brass or cast iron.

**A warning about the Mohs scale.** Mohs is a mineralogist's scratch
ladder built for rocks, and it is far too coarse for metals. In the
project's own alloy file every iron-based entry, from mild steel at 4.0
to Hadfield manganese steel at 8.0, sits in that one band, so the
difference between a knife that holds an edge and one that does not is
invisible at that resolution. Treat a Mohs number
for a metal as a rough ordering, never a specification. This matters for
the data file below, which uses exactly that column.

### Toughness: how much abuse before it cracks

**The energy it takes to break the material**, which is a different
question from the force. The DOE handbook defines it as the work
required to deform one cubic inch of metal until it fractures, measured
by the Charpy or Izod test, both of which swing a hammer at a
**notched** sample. The Army manual's version: toughness is the ability
to resist the start of permanent distortion plus the ability to resist
failure after deformation has begun.

A tough material bends, absorbs and hangs on. A brittle one snaps. Glass
is strong in compression and has essentially no toughness.

### The trade, which is the whole of the subject

Both manuals say it independently.

DOE: as hardness and tensile strength increase in heat-treated steel,
toughness and ductility decrease.

Army: it takes a combination of hardness and toughness to withstand
heavy pounding, and toughness decreases as hardness increases.

**You cannot have both, and every tool is a chosen point on that
trade.** A cold chisel is tempered softer than a file because it gets
hit. A file is harder because it must abrade steel and is never struck.

### Why the cheap knife is either soft or brittle

A knife edge wants two opposing things: hard enough to hold a keen apex,
tough enough that the apex does not chip out.

A cheap knife sits at one end of that trade because hitting the middle
costs money. The middle needs controlled steel chemistry, a controlled
hardening temperature, a controlled quench and a controlled temper, each
held within tens of degrees, using that alloy's own recipe. Err toward
soft and the edge rolls over every few minutes. Err toward hard and it
chips, which is worse, because a rolled edge can be straightened and a
chipped one must be ground back past the damage.

[Sharpening](/library#sharpening) covers what that means at the bench,
including the finding that razor edges often die by chipping rather than
by wearing smooth, and the one colour that says you have ruined a temper
with a grinder.

## Failure modes that surprise people

Everything so far assumed you load a part once and see whether it holds.
Real parts mostly fail in ways a single pull test never predicts.

### Fatigue: failure far below yield, because you did it again

Your paperclip broke at a stress nowhere near what it takes to pull a
paperclip apart. That is what takes out most parts that see any cycling:
engine mounts, springs, shafts, trailer frames, ladder rungs.

The DOE handbook is honest that the cause is not settled: the primary
cause of the phenomenon of fatigue failure is not well known. What it
describes is that failure apparently begins with a small crack at a
defect or a microscopic slip between grains, that the crack propagates
slowly and then faster as the remaining cross section shrinks, and that
the metal then fractures.

Then the part you can act on: **fatigue failure can be initiated by
microscopic cracks and notches, and even by grinding and machining marks
on the surface**, so such defects must be avoided in materials subjected
to cyclic stress.

Read that again. On a part that cycles, a grinding mark across the
direction of stress is a crack starter, not a blemish.

Two consequences. A part that breaks "suddenly" after years usually was
not sudden; the crack had been growing, visibly, and the final fracture
was the last few percent. And a fatigue surface looks different from a
one-pull fracture: a smooth region where the crack crept, and a rough
torn region where the rest gave way. If you can see both zones, fitting
an identical replacement will get you an identical failure.

**One honest gap.** Steels are generally described as having a fatigue
limit, a stress below which they survive indefinitely, while aluminium
alloys are described as having none. That is the standard engineering
position and the reason aircraft have finite lives. **No federal
publication opened for this guide states it**, so it is recorded here as
convention. What is sourced is that NASA's handbooks for 6061 and 7075
publish S-N curves rather than a single safe stress, which is what you
would do for a material with no plateau to quote.

### Stress concentration: the notch does the damage

Stress does not spread evenly. It crowds around holes, sharp internal
corners, thread roots, keyways, weld toes and scratches. A nominal
stress comfortably below yield can be several times higher in a small
volume beside a notch, and that is where the crack starts.

NASA's fastener manual states it precisely for bolts: **if a bolt is
cycled in tension it will normally break near the end of the threaded
portion, because this is the area of maximum stress concentration.** Its
fix sounds backwards: machine the shank down to the root diameter of the
threads. That makes the bolt weaker in a single pull and it survives
cyclic loading much longer, because the stress no longer has a step to
crowd against.

The household version is sharp internal corners: a square-cut notch in a
bracket, a file mark across a shaft, a hole drilled at the edge of a
flange. The remedy is a radius. Round the inside corner, drill a hole at
the end of a slot, blend a weld toe rather than leaving a step. The
radius adds no material where the load is. It stops the load crowding.

### What a connection can do, which is worse than what a material can do

The most instructive failure in the American record was not a materials
failure at all.

On 17 July 1981 two suspended walkways in the atrium of the Hyatt
Regency hotel in Kansas City collapsed. The National Bureau of
Standards, now NIST, investigated. Its executive summary records 113
dead and 186 injured, and calls it in terms of loss of life and injuries
the most devastating structural collapse ever to take place in the
United States.

The steel was not the problem. NBS concluded the most probable cause was
insufficient load capacity of the box beam and hanger rod connections,
and named two contributing factors: the original design of that
connection was inadequate, and a change during construction, replacing
one continuous hanger rod through both walkways with two separate rods,
essentially doubled the load on the fourth floor connection.

The numbers are the part to carry. At collapse, the load on a fourth
floor connection was only **31 percent** of the ultimate capacity that
connection should have had under the Kansas City building code. Even
unchanged, it would have been about **60 percent**. NBS concluded that
with the change, the walkways had from the day of construction only
minimal capacity to resist their own weight and virtually none to resist
the additional load of people.

**This generalises down to a shelf bracket.** Parts rarely fail in the
middle of a bar. They fail where parts meet: at the bolt, the weld, the
hook, the thread, the hanger. Look at the connections first, and ask
what would happen to the load path if one detail were built differently
from the drawing.

### Brittle fracture in the cold

The same steel that bends at room temperature can shatter on a cold
morning. The DOE handbook: many steels show ductile fracture at elevated
temperatures and brittle fracture at low ones, and **the temperature
above which a material is ductile and below which it is brittle is the
nil-ductility transition temperature.**

It is careful about that temperature. It is not precise, it varies with
prior mechanical and heat treatment and with impurities, and it is found
by a drop-weight test such as Izod or Charpy. Two things lower it: small
grain size, and small additions of nickel and manganese to low-carbon
steels.

**In practice, do not shock-load cold steel.** Do not hammer a frozen
splitting wedge or strike a cold chisel outdoors in a hard freeze
without warming it. The hand tool manual that [Sharpening](/library#sharpening)
draws on says the same for axes: a cold blade is brittle and will break
easily. Silverdale's winters are mild, with January means of about 8
degrees C by day and 2 at night, so here this is a cold-snap problem
rather than a daily one. It is not a small one anywhere it is cold.

### Creep: slow failure under a load that never changed

At room temperature a structural material develops the full strain it
will show as soon as the load is applied. The DOE handbook notes this is
not so at high temperature, giving stainless steel above 1000 degrees F
and zircaloy above 500 degrees F as examples: at elevated temperature
and constant load, many materials continue to deform slowly. That is
creep.

It has three stages: primary, where the rate starts high and falls;
secondary, where the rate is small and strain grows very slowly, which
is where a well-designed part spends its life; and tertiary, where the
rate accelerates and strain becomes large enough to fail.

You meet it in stove and flue parts, exhaust hangers, and bolts holding
a hot flange. If a bracket inside a stove has sagged over years without
ever being overloaded, that is creep, not a defect.
[Firewood](/library#firewood) covers the rest of what goes wrong in
a flue, including the creosote that starts chimney fires.

### Hydrogen embrittlement: the one with no warning

The most treacherous failure on the list, because the part looks
perfect, passes a test, and then breaks later under a load it had
already carried.

The DOE handbook gives the mechanism for steel: hydrogen diffuses along
grain boundaries and combines with the carbon to form methane gas, which
collects in small voids and builds up enormous pressures that start
cracks and reduce ductility, so a part under high tensile stress can
fail in a brittle way.

NASA's fastener manual gives the version that matters for hardware you
buy. Most plating is an electrolytic bath process, so free hydrogen is
present at the surface. Most plating therefore requires baking
afterwards to drive the hydrogen out, and for cadmium it gives the
figures: bake at 375 degrees F for 23 hours, within 2 hours after
plating. It then names the trap: heating a plating to its decomposition
temperature can generate free hydrogen again, so **exceeding the safe
operating temperature of a plating can cause premature fastener failure
from hydrogen embrittlement** as well as loss of corrosion protection.
And the reason this one frightens engineers: internal hydrogen
embrittlement can cause delayed failures after proof testing, with **no
external indication that the hydrogen is present**.

So: buy high-strength plated fasteners from a source that bakes them. Do
not electroplate a high-strength bolt in the garage, and do not
acid-pickle a hardened part and then load it. Treat any high-strength
plated fastener that broke without visible deformation as a candidate
for this rather than a fluke.

### Why a ladder rated for 300 pounds fails under less than 300 pounds

Every mode above is in this one question, so it makes a good test of
whether the model is working.

**The rating is a test result, not a promise about your Tuesday.**
Federal construction regulation requires a portable ladder to support at
least four times the maximum intended load, with a reduced 3.3 times for
extra-heavy-duty type 1A metal or plastic ladders. Note how that is
demonstrated: the load is applied **in a downward vertical direction**,
and for a non-self-supporting ladder with it **placed at 75 and a half
degrees from the horizontal**.

So the rating already contains a factor of four, and the ladder still
fails. Why:

- **The rated load is not just you.** The regulation defines maximum
  intended load as the total load of all employees, equipment, tools,
  materials, transmitted loads and other loads anticipated at any one
  time. You, the boots, the belt, the drill, the bundle of shingles.
- **The test load is vertical and the real one often is not.** Leaning
  sideways to reach the last screw loads a component in a direction the
  test never applied.
- **The test load is static.** Stepping down onto a rung applies more
  than your weight for a moment. That is ordinary mechanics rather than
  a figure this guide can cite, so treat the direction as certain and
  the multiplier as unquoted.
- **The ladder was tested new.** Regulation treats damage as
  disqualifying: portable ladders with structural defects including
  broken rungs, split rails, **corroded components** or other defective
  components must be tagged and withdrawn from service until repaired.
  Corrosion sits on that list beside broken rungs because it does the
  same thing.
- **It has been cycled**, and every dent and drilled hole is a stress
  concentration where a crack can start. The handbook's line about
  grinding marks initiating fatigue applies to a scraped aluminium rail
  exactly as it applies to a reactor component.
- **It is aluminium, so it flexes.** At one third the stiffness of
  steel it deflects visibly, which means the material is being worked
  every climb, which is the condition fatigue needs.

The label describes a new ladder, loaded straight down, at the right
angle, by a test machine. Every difference from that spends some of the
factor of four.

## Heat treating plain carbon steel

What the four operations do. Deliberately few temperatures, for the
reason at the top: the numbers belong to specific alloys.

**The precondition.** The Army manual states which steels respond: alloy
steels and plain carbon steels with 0.35 percent carbon or higher can be
hardened to the limits attainable for their carbon content, or softened
as required, by controlling the rates and method of heating and cooling.
It is blunt elsewhere: high carbon steel can be hardened by heating to a
good red and quenching in water, while low carbon steel, wrought iron
and steel castings cannot be hardened. This is the first thing to
establish about unknown steel, and it is why a mild steel bar will never
make a chisel.

**Annealing** is heating above the critical temperature and cooling
slowly. The manual's purposes: to remove stresses, induce softness,
alter ductility, toughness and other physical properties, refine
crystalline structure, remove gases, or produce a definite
microstructure. In workshop terms it is the reset.

**Normalising** is a specific member of that family: heating to
approximately 100 degrees F above the critical range, then cooling in
still air at ordinary temperature. Air rather than furnace cooling gives
a finer, more uniform grain, which is why you normalise a forging or a
weldment before hardening it.

**Hardening** is heating above the critical temperature and cooling
rapidly in water, iced brine or another liquid. The manual explains what
happens: passing up through the critical range, the iron transforms from
a form with low carbon solubility to one with high solubility; on
cooling the reverse occurs, but because these changes take time they can
be arrested by cooling fast enough. If cooling is very rapid, **the
carbon is fixed in a highly stressed, finely divided state, and the
steel becomes hard, brittle and much stronger than steel that is slowly
cooled.**

Hold on to "highly stressed" and "brittle". A just-quenched piece is at
its most fragile, full of internal stress with nowhere to go. Which is
why nobody stops there.

**Tempering**, also called drawing, is reheating the hardened steel
below the transformation range and cooling at any rate. The manual gives
the reason and the rule together: after hardening, a steel is too
brittle for ordinary purposes, so some hardness should be removed and
toughness induced, and **as the tempering temperature increases,
toughness increases and hardness decreases.** It gives the usual range
as 370 to 750 degrees F, sometimes as high as 1,100.

That is the hardness-toughness trade, now under your control, with a
dial marked in degrees. Every tool in your house sits somewhere on it,
chosen by whoever made it.

**The order is not negotiable.** Anneal to a known soft state, shape,
normalise if forged or welded, harden, then temper immediately. A
hardened piece left untempered can crack sitting on the bench.

**One irreversible mistake.** The manual warns that the metal should
never be heated close to its melting point, because certain elements are
oxidised, burned out, and the steel becomes coarse and brittle. **Steel
in that condition usually cannot be restored by any subsequent heat
treatment.** It adds that the lower the carbon content, the higher the
temperature a steel tolerates first, which is another way of saying the
good tool steels are the least forgiving.

**Case hardening** is the answer when you want a hard surface on metal
that cannot harden through. A low carbon steel cannot be hardened much
because of its low carbon, yet the surface can be hardened by increasing
the carbon content of the surface only, for example by pack carburising.
The result is a hard skin over a soft, tough core, which is what a gear
tooth wants. It is also why you must never grind deeply into a
case-hardened part: underneath is mild steel.

### Colours, honestly

Two colour systems get confused and only one is about tempering.

**Glowing colour** is light the steel emits when hot enough to glow, dull
red up through orange and yellow. That is the scale behind "a good red"
for the hardening heat.

**Oxide colour** is different: it appears on clean bright steel at far
lower temperatures as a thin oxide film thickens, and it is the
traditional tempering guide. [Sharpening](/library#sharpening) covers it,
including the one colour two federal manuals name and agree on, blue,
meaning the temper is drawn and the affected metal must be ground away.
That guide also states plainly what could not be sourced: the full
straw, brown, purple sequence everyone quotes appears in none of the
manuals opened for it.

**The limits.** Glowing colour depends heavily on ambient light, which
is why smiths work in shade. Oxide colour depends on surface finish and
on time at temperature as well as peak temperature. Neither says
anything about how far the heat has penetrated. And for one important
metal, colour does not exist at all.

### The hazards, at the operations that carry them

**Quenching.** Hot steel boils the liquid instantly at its surface.
Water flashes to steam and can throw scalding water and hot scale back
at you. Full face protection, not just glasses. Long sleeves and gloves.
A tank deep and wide enough to submerge the part with room to move, that
cannot tip. Keep your face out of the line above it.

**Molten metal and moisture is the serious one.** The Army manual's
thermit welding warning is the clearest federal statement: the mould
must be thoroughly dried before the charge is ignited, and painful burns
may occur from splashing metal, upsetting of the crucible, breaking of
the mould, or **by allowing the molten metal to come in contact with
moisture in the mould.** The mechanism is a quench run backwards and
much larger: water trapped under molten metal becomes steam under the
metal and throws it. Anything being cast into or poured onto must be
dry, including a damp floor, a cold tool laid in the path, and
condensation on a mould left in an unheated shop overnight.

**Hot metal does not look hot.** Steel well below its glow temperature
looks exactly like cold steel, which is the reason for the shop habit of
never picking up a piece you did not see put down. For one
metal it holds all the way up. NASA's aluminium handbook lists it among
four things you must understand to weld the material: **aluminium
exhibits no characteristic colour changes even at temperatures up to the
melting point**, and so temperatures must be controlled by measurement
rather than judged by appearance. Hot aluminium looks like cold
aluminium right until it collapses, which the manual describes as the
metal holding its shape until almost molten and then collapsing
suddenly.

Assume every piece of metal in a hot workshop is hot. If a burn happens
the first minutes decide the outcome, and
[Treating Burns](/library#treating-burns) gives the cooling numbers
and the criteria for getting help.

## Corrosion, where most household metal actually dies

Very few things fail by being overloaded. They fail because they rusted,
or because they touched something they should not have.

### The battery in your fence post

The DOE chemistry handbook: **galvanic corrosion is the corrosion that
results when two dissimilar metals with different potentials are placed
in electrical contact in an electrolyte.**

All three conditions are required. Two different metals, electrical
contact, and an electrolyte, which domestically means water with
anything dissolved in it, salt water most of all.

The mechanism: a potential difference between the metals drives a
current through the electrolyte, and that current corrodes one of them.
The less resistant, more active metal becomes the anode; the more noble
metal is cathodic and protected. Then the line that makes it vivid:
**with no electrical contact, the two metals would be uniformly attacked
by the corrosive medium as if the other metal were absent.** Bolting
them together creates the battery. The larger the potential difference,
the greater the probability.

### The ranking, and how to use it

NASA's fastener manual publishes a galvanic ranking of common
engineering materials from most active to least, with the note that
**the farther apart two materials are in the list, the greater the
galvanic action between them.** Abridged to what you are likely to hold,
keeping the manual's own position numbers:

| Position | Material |
|---|---|
| 1 | Magnesium (most active) |
| 3 | Zinc |
| 4 | Aluminium 5056 |
| 5 | Aluminium 5052 |
| 6 | Aluminium 1100 |
| 8 | Aluminium 2024 |
| 9 | Aluminium 7075 |
| 10 | Mild steel |
| 11 | Cast iron |
| 13 | Type 410 stainless (active) |
| 14 | Type 304 stainless (active) |
| 15 | Type 316 stainless (active) |
| 16 | Lead |
| 17 | Tin |
| 21 | Yellow brass |
| 25 | Copper |
| 26 | Silicon bronze |
| 30 | Titanium |
| 31 | Monel |
| 32 | Type 304 stainless (passive) |
| 33 | Type 316 stainless (passive) |
| 34 | Silver |
| 35 | Graphite |
| 36 | Gold (least active) |

The gaps are entries left out here for brevity; the full 36-entry list
is in the source.

Two things there are easy to miss and both matter.

**Stainless appears twice, far apart.** The manual explains: passivation
is done by oxidising in an air furnace or treating the surface with acid
to form an oxide, and this oxide surface is quite inert and deters
galvanic activity. Passive 304 sits at 32, near gold. Active 304, the
same alloy with its film destroyed or starved, sits at 14, near mild
steel. **The same stainless bolt is a different metal electrically
depending on whether its film is intact.**

**Graphite is on the list at 35, more noble than silver.** Graphite
grease, pencil marks and carbon fibre will all drive corrosion in any
metal they touch in the wet.

### The area rule, which decides how bad it gets

The manual gives the most useful single sentence in this section:
**because the anode is eroded in a galvanic cell, it should be the
larger mass in the cell.** Its own conclusions follow: it is poor design
practice to use carbon steel fasteners in a stainless steel or copper
assembly, while stainless steel fasteners can be used in carbon steel
assemblies, since the carbon steel mass is the anode.

The same current flows either way, but it is spread over the whole
anode. A large anode loses a film nobody notices. A small anode loses
the same metal from a tiny volume and disappears.

So the rule for anything you assemble: **the small part should be the
more noble one.** Stainless screws into an aluminium or steel frame, not
aluminium rivets into a stainless frame. A brass valve on a long steel
pipe is tolerable; a short steel nipple screwed into a large brass
manifold will be eaten.

### The classic pairings, including one usually stated wrong

**Copper against aluminium is the unambiguous disaster.** Positions 25
against 4 to 9, and in the usual arrangement the copper is the small
noble part. A copper pipe strapped to an aluminium bracket, a copper
earthing conductor bolted to an aluminium chassis, or runoff from a
copper roof dripping on aluminium gutters will all eat the aluminium.

**Stainless against aluminium in salt** is the marina classic. Passive
stainless at 32 against aluminium at 4 to 9 is nearly the worst
separation in common hardware. With a small bolt through a large plate
the area rule is on your side and the attack spreads. What ruins it is
the crevice under the washer, below, which starves the film of oxygen
and concentrates the attack exactly where the metals meet.

**Galvanised against copper** is real and fast. Zinc at 3 against copper
at 25 is nearly the full span of the list, and the zinc goes quickly. A
galvanised tank fed by copper pipe is a short-lived arrangement.

**Galvanised against aluminium is usually stated backwards, and the
correction is worth having.** You will hear that a galvanised bracket
eats the aluminium it is bolted to. While the zinc is intact the series
says the opposite: zinc at 3 is more active than every aluminium on the
list, so the zinc corrodes in preference and protects the aluminium.
That is the point of galvanising, and NASA says the same of the coating:
zinc is a sacrificial material and will migrate to uncoated areas that
have had their plating scratched off, continuing to provide protection.

**The trouble starts when the zinc is gone.** Underneath is mild steel
at 10, more noble than every aluminium on the list. The roles swap, the
aluminium becomes the anode, and a small cathode of exposed steel
concentrates the attack right at the joint. So the observation people
report is real; the mechanism is that the galvanising ran out, and the
timing depends on how thick the zinc was and how wet the joint is. In a
dry indoor joint this may never happen. On the salt-water side of a
Kitsap winter it happens sooner than people expect.

### Sacrificial anodes: doing it on purpose

The DOE chemistry handbook describes the deliberate version: cathodic
protection by attaching a third metal with an even greater oxidation
potential, so that **the most active metal tends to corrode in place of
the protected metal.** That is a sacrificial anode, and zinc is a common
one, often used in cooling water systems containing seawater.

This is why boats and water heaters carry anodes meant to dissolve. An
anode still looking new after years is not good health. It means it is
not electrically connected to what it should be protecting, and the
protected metal is corroding instead.

### Why stainless is stainless, and the two ways to defeat it

Stainless does not resist rust by being inert. It resists rust **by
rusting instantly and then stopping.**

The DOE chemistry handbook: passivity is shown when a metal does not
become active in the corrosion reaction, and it is caused by the buildup
of a stable, tenacious layer of metal oxide formed by corrosion on a
clean surface, where the corrosion products happen to be insoluble in
that environment. Once formed, it is a barrier, and for corrosion to
continue the reactants must diffuse through it, which is very slow or
does not happen. It names the metals that do this on exposure to air or
pure water at room temperature: zirconium, chromium, aluminium and the
stainless steels, adding that the film may be invisible to the unaided
eye and still very effective. The Army manual says the same from the
other direction: the stainless family's properties **are due to the
formation of a very thin oxide film on the surface of the metal.**

A stainless part is a thin ceramic film on a steel that would otherwise
rust briskly. Two things destroy it.

**One: chlorides.** The DOE handbook's prevention list names them first,
avoiding agents in the medium that cause pitting, for example chlorides
and oxygen. It is more specific about cracking: stainless steels
containing 18 percent chromium and 8 percent nickel, which is the 304
family, **are susceptible to cracking in environments containing
chloride ions** and in concentrated caustic environments, while showing
no such tendency in water containing nitrate, sulfite or ammonium ions.
It is specifically chloride.

That is why the same stainless behaves differently in two places. In a
kitchen, chloride exposure is intermittent and gets rinsed. In a
boatyard, salt spray coats everything and dries, leaving concentrated
chloride sitting on the film continuously. The alloy did not change; its
environment did. This is also what the marine grade is about: the
project's alloy file describes `stainless_316` as marine grade whose
molybdenum adds pitting resistance, and chloride pitting is the attack
it is resisting. **Note that the molybdenum link is the data file's
claim, not a federal one**; none of the federal sources opened for this
guide states it, though the DOE chemistry handbook is unambiguous that
chloride is the agent doing the damage.

**Two: oxygen starvation, which is crevice corrosion.** The film needs
oxygen to repair itself. Where oxygen cannot reach, the film is not
maintained, the metal goes active, and it corrodes fast in a tiny area.

The DOE handbook is direct about the risk: pitting and crevice corrosion
are a major hazard because of the **rapid penetration of the metal with
little overall loss of mass.** The part does not look corroded. It looks
fine, and it has a hole through it. Pitting needs low flow plus areas of
both high and low oxygen concentration, which set up a differential
aeration cell, and crevice corrosion is the version that happens
specifically in the low-flow region of a crevice.

**In a house the crevices are:** under a washer, under a gasket, under a
sticker left on a stainless sink, in the thread of a fastener, between
two plates bolted face to face, under a rubber mount, under standing
water in a pressed dish, and inside any tube that does not drain. The
handbook's four preventions are avoiding stagnant or low flow
conditions, choosing less susceptible alloys, avoiding chlorides and
oxygen differences, and **designing so that no crevices are present.**

That last is a design instruction rather than a maintenance one. Slope
surfaces so they drain. Do not trap water under a flange. Seal a joint
completely or leave it completely open, because nearly sealed is the
worst of both.

**Stress corrosion cracking** is the third of the family: intergranular
attack at the grain boundaries under tensile stress. What makes it
dangerous is that it is relatively independent of general corrosion, so
**general corrosion can be essentially nil and stress cracking can still
occur.** It needs three things together: a susceptible alloy, a specific
environment, and tensile stress. Where the environment is severe,
cracking can occur in minutes.

NASA adds the version relevant to hardware you buy: an otherwise ductile
part will fail far below its yield strength because of surface
imperfections created by the corrosive environment, and **in general,
the higher the heat-treating temperature and the lower the ductility,
the more susceptible it is.** This is why the strongest fastener is not
always the right one.

Two more mechanisms, defined in the FAA's inspection and repair
circular. **Fretting corrosion** is corrosion damage between
close-fitting parts allowed to rub together, where the rubbing prevents
the formation of protective oxide films. It looks like red-brown powder
emerging from a joint that is supposed to be tight, and it means the
joint has been moving; tightening it is the fix, wiping it is not.
**Filiform corrosion** is a thread-like corrosion forming on aluminium
skins beneath the finish, looking like worm tracks under paint, meaning
moisture has got under the coating.

### Silverdale specifically

The canonical locale sits at the head of Dyes Inlet, an arm of Puget
Sound on the Kitsap Peninsula. Three facts from the local data decide
what corrodes here.

**It is salt water**, which puts chloride onto every outdoor surface
within reach of spray. That is the specific ion that defeats stainless,
so the 316 versus 304 distinction is not over-engineering here.

**It is wet for months.** The locale climate record gives 1,446 mm of
precipitation a year, with 236 mm in January alone. Between roughly
October and April an electrolyte is not occasional; it is the default
state of every outdoor joint.

**It is mild.** January means of about 8 degrees C by day and 2 at night
mean outdoor metal spends the wet season above freezing, and water that
stays liquid keeps corroding. A hard continental winter at least pauses
the reaction; this one does not.

So: choose 316 near the water, isolate dissimilar metals with a
non-conducting washer or coating rather than trusting paint, and design
joints that drain rather than joints that hold salt water against a
crevice all winter.

## The common metals, and what each is for

**Plain carbon steel.** Iron with carbon, and the carbon decides almost
everything. The Army manual's bands: **low carbon, up to 0.30 percent**,
soft and ductile, works hot or cold, welds readily by all methods, and
does not harden appreciably when quenched. This is structural steel,
angle, tube, sheet and most bolts. **Medium carbon, 0.30 to 0.45
percent**, heat treatable after fabrication, with the warning that the
weld zone will harden if cooled rapidly and must be stress relieved.
**High carbon, 0.45 to 0.90 percent**, tool material, supplied annealed
so it can be machined first, and difficult to weld because of the
hardening effect at the joint. **High carbon tool steels, 0.60 to 1.70
percent**, where high hardness is needed to keep a cutting edge.

**Alloy steels** change hardenability and properties. The manual's
summary of the common additions: chromium increases hardenability,
corrosion resistance and shock resistance and gives high strength with
little loss of ductility; nickel increases toughness, strength and
ductility and lowers the hardening temperature so an oil rather than
water quench is used; manganese gives greater toughness and wear
resistance but decreases weldability as it rises; molybdenum increases
hardenability, meaning the depth to which hardening penetrates, improves
impact fatigue up to about 0.60 percent and impairs it above that.

**Stainless steels.** The 300 series, 304 the workhorse and 316 the
marine version, is austenitic: tough, very formable, and **it cannot be
hardened by heat treatment.** The DOE handbook notes exactly this of a
reactor tank, that because of the crystal pattern of type 304, heat
treatment is unsuitable for increasing hardness and strength. You
strengthen it by working it. The 400 series is different metal wearing
the same name: less chromium, and it hardens like a carbon steel, which
is why knife blades use it. NASA warns of the cost: series 400 contains
only 12 percent chromium and thus will corrode in some environments. A
400-series blade buys hardness with corrosion resistance.

**Cast iron.** So much carbon that some appears as free graphite. The
Army manual gives total carbon of 1.7 to 4.5 percent, and for commercial
grey iron 2.5 to 4.5 percent, of which about 1 percent is combined and
about 2.75 percent stays free. Those graphite flakes lubricate, which is
why a cast iron pan and a cast iron machine way both slide well. They
also act as thousands of internal notches, which is why cast iron has
almost no toughness: it **breaks short when fractured**, and small
brittle chips made with a chisel break off as soon as they form. Ductile
iron is the same chemistry with the graphite forced into spheres,
removing most of those notches. White cast iron has its carbon combined
rather than free, and is very hard and very brittle. Do not shock-load
grey cast iron: a dropped cast iron pan cracks where a steel one dents.

**Aluminium.** Light, soft, low strength, easily cast, forged, machined
and welded, and suitable only in low temperature applications except
when alloyed. **It cannot be hardened the way steel is**, and this is the
most misunderstood thing about it. There is no carbon to trap, no
critical temperature to quench through, and no tempering colour to
watch. There are two entirely different routes instead.

*Work hardening.* The 1xxx, 3xxx and 5xxx families are not
heat-treatable, and the only way to strengthen them is to deform them
cold. Their tempers carry an H, as in 5052-H32.

*Precipitation hardening, also called age hardening*, and it is nothing
like quench-and-temper. NASA's 6061 handbook gives the sequence:
solution treat at roughly 516 to 545 degrees C depending on product
form, quench rapidly in cold water, then age, either at room temperature
for 96 hours to reach T4 or at 171 to 182 degrees C for 7.5 to 8.5 hours
to reach T6. Annealing is different again, 413 degrees C held 2 to 3
hours, slow-cooled.

Notice what that does. **The quench does not harden aluminium; it leaves
it soft and supersaturated.** The hardening happens afterwards, slowly,
as fine particles precipitate out of the solid. Quench a piece of 6061
and it will be soft on Monday and harder on Friday having done nothing.
That is the opposite of steel.

Notice how narrow the window is. The project data file gives 6061 a
melting point of 855 K, which is 582 degrees C. The top of the solution
treatment range is about 545. That leaves **under 40 degrees C** between
doing it right and melting the part, and aluminium gives no colour
warning at all as it approaches that line. This is furnace work with an instrument,
not eyeball work.

What the treatment buys, and what it costs:

| Alloy and temper | Ultimate | Yield | Elongation in 2 in |
|---|---|---|---|
| 6061-O (annealed) | 18.0 ksi (124 MPa) | 8.0 ksi (55 MPa) | 30 percent |
| 6061-T6 | 45.0 ksi (310 MPa) | 40.0 ksi (276 MPa) | 17 percent |
| 7075-O (annealed) | 33.0 ksi (228 MPa) | 15.0 ksi (103 MPa) | 17 percent |
| 7075-T6 | 83.0 ksi (572 MPa) | 73.0 ksi (503 MPa) | 11 percent |

Ultimate rises by about 2.5 times in both alloys, and in both the
elongation, the measure of how far it stretches before parting, falls by
more than a third: 30 percent to 17 in the 6061, and 17 to 11 in the
7075. **The strength was bought with ductility**, as the ripple
model predicts, and the modulus did not move. NASA also lists 7075 as
resistant to stress corrosion cracking in the T73 temper, a deliberately
over-aged, slightly weaker condition: **a temper is a choice about which
failure mode you would rather have.**

**Copper, brass and bronze.** Copper is ductile, malleable, and a fine
conductor of heat and electricity. The manual notes that pure copper is
not suitable for welding and is difficult to machine because of its
ductility, and that it oxidises to various shades of green. **Brass is
copper plus zinc; bronze is copper plus tin**, and the manual
immediately warns the naming is unreliable: many bronzes contain more
zinc than tin and some contain zinc and no tin at all. Do not trust the
name on a fitting. In high brasses, 20 to 45 percent zinc, tensile
strength, hardness and ductility all rise with the zinc. Copper-nickel
alloys at 10, 20 or 30 percent nickel are moderately hard, tough and
ductile, and **very resistant to high velocity seawater, stress
corrosion and corrosion fatigue**, which is why they appear in marine
plumbing. **Beryllium copper** needs its own warning: 1.5 to 2.75
percent beryllium, ductile when soft, gaining tensile strength when age
hardened, and superb for springs and non-sparking tools. It is also the
most dangerous common alloy to grind or machine, for reasons in the
hazards below.

**Lead.** Heavy, soft, malleable, low melting point, low tensile and low
creep strength, resistant to ordinary atmosphere, moisture and water,
and particularly effective against many acids. **All of that is true and
none of it is a reason to choose it.** Lead is a cumulative poison. Old
lead pipe, flashing, paint and plumbing solder are things you may
encounter in an existing building; encountering them is different from
choosing them. Its hazard entry below is not a formality.

**Zinc** is medium-low strength with a very low melting point, and its
main structural role is as a coating. Hot-dip zinc coating is what
galvanising means. NASA gives its limits: zinc plating has a useful
service temperature limit of 250 degrees F, and its corrosion-inhibiting
qualities degrade above 140 degrees F.

**Magnesium** is the lightest structural metal, and two things matter.
Galvanic corrosion is an important factor in any assembly with
magnesium, which the NASA list confirms by putting it at the very top of
the active end: it will corrode in preference to absolutely everything
else present. NASA's instruction is that magnesium must be totally
insulated from fasteners by an inert coating such as zinc chromate
primer, and that if the coating is damaged, cadmium or zinc plated
fasteners are the most compatible because they are closest to magnesium
in the series. Second, and useful in the shop, magnesium is
distinguished from aluminium with silver nitrate solution, which does
not affect aluminium but leaves a black deposit of silver on magnesium.
Magnesium chips and dust burn fiercely and are not put out with water.

## Joining, and why the weld fails beside the weld

Four ways to hold metal together, organised by how hot the parent metal
gets and whether it melts.

**Welding melts the parent metal.** The joint is continuous parent
material. Strongest joint, most damage to the surrounding metal.

**Brazing does not melt the parent metal.** A lower-melting filler flows
into the joint by capillary action and bonds to both surfaces. The Army
manual describes silver brazing alloys as silver with varying
percentages of copper, nickel, tin and zinc, used for all ferrous and
nonferrous metals except aluminium, magnesium and others melting too
low. Joints must be free of oxides, scale, grease and dirt, and flux is
generally required, melting below the filler.

**Soldering is the same idea much cooler**, with much weaker filler. A
soldered joint is a sealed joint, not a structural one.

**Mechanical fastening does not heat the metal at all**, which is its
great advantage: bolts, rivets and screws leave the temper untouched.
What they do instead is drill holes in it, and every hole is a stress
concentration.

### The heat-affected zone

This is why a weld cracks next to the weld.

The Army manual defines it: **the heat affected area is that portion of
the base metal that is changed metallurgically by the welding heat**,
and it has three zones, the very hot section next to the molten filler,
the annealed section next to the overheated base metal, and the zone
adjacent to the cold base metal.

Read the middle zone again. **Next to your weld is a strip of metal that
has been annealed**, softened by heat that was not enough to melt it.
The weld is fresh, sound, fully melted metal. The parent metal well away
is untouched. In between is a band whose properties were changed by an
uncontrolled heat treatment nobody designed, bounded on both sides by
metal with different properties, so stress concentrates there.

For steel the concern is the opposite of softening. Arc welding produces
greater hardness in the heat affected area than gas welding, and **the
greater the hardness produced, the more likely the weld is to crack when
the molten metal solidifies.** The manual is quantitative about where
trouble begins: arc welds on plate containing 0.35 percent carbon or
higher show a greater rate of hardness increase than steels with less,
and the carbon content of readily weldable grades is therefore kept low
deliberately. In plain carbon steels at 0.25 percent carbon or less,
welds by either arc or gas show no noticeable change in hardness,
ductility or tensile strength.

**That is the weldability rule in one line: low carbon steel welds
because there is not enough carbon to harden the zone beside the weld.**
Above about 0.35 percent that zone hardens on cooling, turns brittle and
cracks. The remedies are preheating to slow the cooling, and stress
relieving afterwards.

For heat-treated aluminium the concern is softening, and NASA's 6061
handbook has the direct evidence. Testing welded 6061, it reports that
**all specimens in that study failed at the edge of the weld or in the
annealed zone areas which do not respond to ageing.** It also measured
the reach: the effect of welding heat did not extend more than 1.5
inches, 38.1 mm, from the weld centreline.

Those two sentences are the whole thing. Weld a 6061-T6 frame and you
get a band roughly an inch and a half either side of every weld that has
been annealed back toward the soft condition, and that does not recover
by sitting there, because the precipitation that gave it strength needs
a proper solution treatment and age. The weld metal is fine. The soft
band beside it is where it breaks. This is why welded aluminium frames
and boat fittings are either heat treated after welding or designed with
extra section at the joints, and why an amateur repair weld on a T6
extrusion often fails about an inch from the repair.

### Bolts, and why one shears

NASA's fastener manual names most of the mechanisms.

**Threads in the shear plane.** Standard design practice is to choose a
grip length such that **the threads are never in bearing**, that is,
never in shear. A bolt in shear should present its plain shank to the
joint, not its thread, which is a smaller diameter and a stress
concentration at once. Where the available grip length is wrong, the fix
is to vary the washer thickness under the head or nut.

**Loss of preload.** In a normal clamped joint the clamped faces are
much stiffer than the bolt, so the bolt load does not rise much as
external load is applied, and does not rise significantly until the
external load exceeds the preload. **A tight bolt is shielded from the
cyclic load; a loose bolt takes all of it.** The commonest reason a bolt
fails in fatigue is that it was not tight.

**Tightening is far less accurate than people think.** The manual's tool
accuracy table gives preload accuracy of only plus or minus 15 to 30
percent when controlling by torque. Measuring bolt stretch directly gets
to plus 1 to plus 8 percent. A torque wrench is not a precision preload
instrument.

**It may not be the bolt you think it is.** The manual documents
counterfeit fasteners, the best-documented case being grade 8.2 boron
bolts deliberately marked as grade 8. Grade 8.2 is a low-carbon, 0.22
percent, boron alloy heat treated to the same room-temperature hardness
as grade 8 medium-carbon, 0.37 percent, steel, but its strength drops
drastically above 500 degrees F where grade 8 serves to 800. Identical
markings, identical hardness test, different behaviour when hot.

**More small bolts beat fewer big ones.** For fatigue, using more
smaller-diameter fasteners rather than a few large ones gives a more
fatigue-resistant joint. For strength generally, it is preferable to use
more fasteners of ordinary strength than a few high-strength ones,
because fasteners above 180 ksi bring brittleness, critical flaws and a
need for stringent quality control.

[Working With Wood](/library#working-with-wood) treats the other half of the
problem: a bolt that will not shear is no use in a member that splits.

### Hazards, at the operation that carries them

**Heating or cutting anything galvanised, and any brass.** Zinc fume
causes metal fume fever. The NIOSH entry for zinc oxide lists chills,
muscle ache, nausea, fever, dry throat and cough, along with weakness,
metallic taste, headache, blurred vision, low back pain, vomiting, chest
discomfort and difficulty breathing. The NIOSH limit for the fume is 5
mg/m3 as an eight-hour average with a 10 mg/m3 short-term limit, and 500
mg/m3 is immediately dangerous. Federal regulation requires local
exhaust or mechanical ventilation indoors for welding or cutting
zinc-bearing or zinc-coated metal, and full confined-space provisions in
a confined space. **Outdoors, upwind, with the coating ground off
first, is the amateur's version.** The illness is usually self-limiting,
which is exactly why people keep doing it.

**Anything cadmium plated, and cadmium-bearing silver brazing alloys.**
This one is not self-limiting. NIOSH lists cadmium as a potential
occupational carcinogen with prostate and lung as the cancer sites,
gives an OSHA limit of 0.005 mg/m3, and lists pulmonary oedema,
difficulty breathing, cough, chest tightness, emphysema, proteinuria and
mild anaemia, with the respiratory system, kidneys, prostate and blood
as target organs. Federal regulation requires local exhaust or airline
respirators indoors or in confined spaces, and approved respirators even
outdoors. The Army manual adds the brazing warning: grind all cadmium
surfaces back to base metal first, because **cadmium oxide formed by
overheating and melting of the silver brazing alloys is highly toxic.**
Old plated hardware is often cadmium and looks like zinc. If you cannot
tell, do not heat it.

**Anything containing lead, including old paint and old solder.** The
NIOSH limit is 0.050 mg/m3, and the effects listed include weakness,
insomnia, pallor, loss of appetite, weight loss, constipation, abdominal
pain and colic, anaemia, a lead line on the gums, tremor, wrist and
ankle paralysis, encephalopathy, kidney disease and hypertension, with
the central nervous system, kidneys and blood among the target organs.
Federal regulation requires local exhaust or airline respirators for
cutting or welding metals containing lead other than as an impurity, or
coated with lead-bearing materials **including paint**, and approved
respirators even outdoors. The route that catches people at home is not
the fume. It is dust on hands, then food.

**Beryllium copper, the one to be genuinely careful with.** The OSHA
limit for beryllium is 0.0002 mg/m3 as an eight-hour average, with a
0.002 mg/m3 short-term limit. Against 0.050 mg/m3 for lead, **the
beryllium limit is 250 times lower.** NIOSH designates it a potential
occupational carcinogen with lung cancer as the site and lists chronic
beryllium disease, with weight loss, weakness, chest pain, cough and
pulmonary insufficiency. Federal regulation is correspondingly strict:
welding or cutting beryllium-containing metals requires local exhaust
ventilation **and** airline respirators, indoors, outdoors or in
confined spaces, unless atmospheric testing under the most adverse
conditions shows exposure is acceptable. Note that outdoors is not an
exemption here as it is for zinc. Do not grind, sand, machine or torch
beryllium copper. The non-sparking tools it makes are safe to use and
hazardous to shape.

**Confined spaces, for all of the above.** Federal regulation requires
adequate ventilation to prevent accumulation of toxic materials or
oxygen deficiency, applies that to helpers and bystanders as well as the
welder, requires a worker stationed outside, and states in a line worth
memorising because the mistake is fatal that **oxygen shall never be
used for ventilation.**

**Grinding sparks and dust** are covered in [Sharpening](/library#sharpening),
including the federal requirements for abrasive wheels and the silica
limit for stone dust. The metal-specific addition is that grinding is
how plating comes off, so grinding a plated part liberates exactly the
coating the welding rules above are written about.

## Identifying an unknown metal with what you have

How far each ordinary test gets you, and where it stops.

**A magnet.** The fastest single test. Strongly magnetic means
iron-based: carbon steel, alloy steel, cast iron, wrought iron, or a
400-series stainless. Not magnetic means aluminium, copper, brass,
bronze, lead, zinc, titanium, or an annealed 300-series stainless.
**Where it stops:** it does not distinguish mild steel from tool steel,
which is usually the distinction you need, and cold-worked 304 can be
mildly magnetic, so a weak response settles nothing.

**Mass and volume, for density.** Weigh it, find its volume by water
displacement, divide. Genuinely discriminating across families:
aluminium near 2.7 g/cm3, titanium near 4.5, steel near 7.85, copper
alloys near 8.5, lead near 11.3. **Where it stops:** it cannot tell
steels apart at all: every steel in the project's alloy file, from D2
tool steel at 7.70 to high-speed steel at 8.16, sits inside a spread of
6 percent. It also fails on anything hollow, plated or thickly painted.
[Units and Converting Them](/library#units-and-converting-them)
covers the arithmetic and how many digits of it you are entitled to
keep.

**A file.** Draw a sharp file across an inconspicuous corner. Cuts
easily means soft; skates means hard. This is the most useful hardness
test available without equipment, and it is how you tell an annealed
tool steel from a hardened one. **Where it stops:** two categories, not
a number, and a case-hardened part skates at the surface and cuts like
butter a millimetre in.

**Colour.** Copper reddish brown, oxidising to green. Brass yellow.
Bronze browner than brass. Aluminium light grey to silver, bright
polished and dull oxidised. Lead smooth grey-white when freshly cut,
oxidising quickly to dull grey. Zinc blue-white polished, oxidising to
grey. **Where it stops:** plating defeats it entirely, and the
brass-versus-bronze call by eye is not reliable, as the manual's own
warning about the naming makes clear.

**Sound and fracture.** Cast iron rings dull; steel rings. Nick a corner
with a chisel and strike it: grey cast iron shows a dark grey surface
from fine black graphite specks and breaks short with brittle chips;
wrought iron breaks jagged because of its fibrous slag structure; low
carbon steel shows bright crystalline and is tough when chipped; high
carbon steel is very fine grained and whiter. **Where it stops:** you
have to damage the part, and the distinctions take practice.

**The spark test.** Touch the metal to a grinding wheel in shadow. The
Army manual's descriptions are detailed enough to use:

- **Wrought iron:** straw-coloured sparks near the wheel, changing to
  white forked sparklers near the end of the stream.
- **Grey cast iron:** a small volume of dull red sparks in a straight
  line close to the wheel, breaking into many fine repeated spurts that
  turn straw coloured.
- **Low carbon steel:** long yellow-orange streaks, brighter than cast
  iron, tending to burst into white forked sparkles.
- **High carbon steel:** a large volume of brilliant yellow-orange
  sparklers; tool steel shows a moderately large volume of white streaks
  with many fine repeating bursts.
- **18-8 stainless:** similar to wrought iron but only half as long.
- **Chromium in quantity:** shortens the stream to about half the length
  of the same steel without chromium, without much affecting brightness.
- **Molybdenum:** a characteristic detached arrowhead, visible even
  through fairly strong carbon bursts.
- **Nickel:** a short, sharply defined dash of brilliant light just
  before the fork, recognisable only when carbon is low enough that the
  bursts are not prominent.
- **Aluminium, copper, lead, zinc and titanium:** no sparks at all.

**Where the spark test stops, and this matters.** It identifies family
and rough carbon content, not grade. The manual's own manganese example
proves the limit: a steel with 0.55 percent carbon and no alloying
elements sparks the same as one with 1.60 to 1.90 percent manganese. Two
different metals, one spark pattern. And it says nothing whatever about
temper, which is usually what decides whether the part will work.

**The honest summary.** With a magnet, a scale, a file and a grinder you
can sort metal into families and tell hard from soft. You cannot
determine a grade and you cannot determine a temper. If the answer
matters, because the part is structural or because you are about to heat
treat it, the correct move is to buy known stock with a certificate
rather than identify mystery metal. Every experienced shop does this,
which is a stronger argument than any test above.

## Reading the data file: data/chemistry/alloys.csv

The simulation carries 74 alloys in `data/chemistry/alloys.csv`.
Learning to read it transfers, because it has the virtues and the limits
of every materials table you will meet.

Eleven fields. **`id`** and **`name`** are the handle and the label.
**`composition`** gives elements and percentages separated by a pipe, as
in `Fe:68|Cr:16|Ni:10|Mo:2|Mn:2`, and it is the recipe that makes the
rest possible. **`density_g_cm3`** is the one property you can verify
yourself with a scale and a bucket of water. **`melting_point_k`** is
kelvin, so subtract 273.15 for degrees C.
**`tensile_strength_mpa`** is ultimate tensile strength, the point where
it parts. **`hardness_mohs`** is the mineral scratch scale, 1 to 10.
**`thermal_conductivity`** and **`electrical_conductivity`** carry no
units in their names; the values are consistent with watts per metre per
kelvin and siemens per metre, but the file does not say so.
**`corrosion_resistance`** is a word, not a number.
**`description`** is free text.

### What a row can tell you

A lot, read as chemistry rather than as a specification.

`stainless_316` is `Fe:68|Cr:16|Ni:10|Mo:2|Mn:2`, corrosion resistance
`very_high`, described as marine grade with molybdenum adding pitting
resistance. Everything in the corrosion section above is visible there:
the chromium forms the passive film, the molybdenum is the addition that
helps against chloride pitting. Compare `stainless_304`,
`Fe:71|Cr:18|Ni:8|Mn:2|C:0.08`, with no molybdenum, and you can see what
you are paying for at the chandlery.

`stainless_440c` is `Fe:80|Cr:17|C:1.1|Mo:0.75`, hardness 6.0 against
5.5 for 304, described as high-carbon martensitic stainless for knife
blades. The 1.1 percent carbon is the whole story: it is what lets the
alloy harden, per the 0.35 percent threshold above, and it is why its
corrosion resistance is `medium` rather than `high`. The knife alloy
trades corrosion resistance for the ability to take a temper, in one
line of a CSV.

The thermal conductivity column reads against household experience too.
`carbon_steel` is 51.9 and `stainless_304` is 16.2, roughly a third,
which is why a stainless pan heats unevenly. Both are still enormous
next to building insulation, which is why a metal fastener through a
wall is a thermal bridge, treated in
[Insulation and Heat Loss](/library#insulation-and-heat-loss).

### What a row cannot tell you, which matters more

**A row is a point, not a grade.** `carbon_steel` is written `Fe:99|C:1`
while its own description says 0.5 to 1.5 percent carbon. The real
material is a range, and where in that range your bar sits decides
whether it can be hardened.

**A row cannot tell you the condition, and this is the big one.** The
same alloy in different tempers is effectively different materials, and
the file has one strength column and no temper column.

Work it through. The file gives `aluminum_6061` 310 MPa. NASA gives 6061
an ultimate of 45.0 ksi at T6; 45,000 psi times 0.006894757 is 310.3
MPa, so the file's number is the T6 value. The same handbook gives the
annealed condition as 18.0 ksi, which is 124 MPa. **The file's number is
2.5 times the annealed strength of the same alloy, and nothing in the
row says so.**

Check against a second alloy. The file gives `aluminum_7075` 572 MPa.
NASA gives 83.0 ksi at T6, which is 572.3 MPa, and 33.0 ksi annealed,
which is 227.5 MPa. Same pattern, same ratio of about 2.5.

So the file consistently records the strong temper. That is a defensible
choice; what makes it a trap is that the row does not announce it. If
you annealed some 6061 to bend it and then sized a part from 310 MPa,
you would be over by a factor of 2.5.

**A row cannot tell you about stiffness, and stiffness decides whether a
thing sags.** There is no modulus column, and nothing in the file
reveals that the two steels are interchangeable for deflection and the
two aluminiums are interchangeable for deflection.

**A row cannot tell you about yield, which decides whether a thing bends
permanently.** There is no yield column, and the gap between yield and
ultimate is not constant. NASA gives 6061-O a yield of 8.0 ksi against
an ultimate of 18.0, a ratio of 0.44, and 6061-T6 a yield of 40.0
against 45.0, a ratio of 0.89. **A T6 part yields at 89 percent of its
breaking stress and an annealed one at 44 percent**, so the same tensile
number means very different things about warning before failure.

**A row cannot tell you about toughness or ductility.** There is no
elongation column, so nothing distinguishes 7075-T6 stretching 11
percent before parting from 6061-O stretching 30 percent. For anything
shock-loaded, that was the property you wanted.

**A row cannot tell you which of two metals will be eaten when you bolt
them together**, because there is no galvanic position column. The
`corrosion_resistance` word describes a metal alone; galvanic corrosion
is a property of a pair.

**A row cannot tell you whether the metal is poisonous.**
`beryllium_copper` reads `Cu:98|Be:2`, described as non-sparking tools,
springs, highest strength copper alloy, corrosion resistance `good`.
Nothing hints that its dust has a limit 250 times lower than lead's.
`wood_metal` reads `Bi:50|Pb:25|Sn:12.5|Cd:12.5`, over a third of its
mass lead and cadmium, and is described only by its melting point and
its use in sprinkler links. You have to read the composition and know
the elements.

### Data defects found while writing this guide

Reported, not fixed, and each checkable against the file itself.

**1. `corrosion_resistance` uses two overlapping vocabularies at once.**
Across the 74 rows: `excellent` 21 times, `medium` 20, `low` 19, `good`
12, `very_high` once, `high` once. One ladder runs low, medium, high,
very_high and another runs low, medium, good, excellent, mixed in one
column. The only row using `high` is `stainless_304` and the only one
using `very_high` is `stainless_316`. There is no way to know whether
`good` on `aluminum_6061` ranks above or below `high` on
`stainless_304`, which makes the column unsortable.

**2. `manganese_steel` has a hardness that contradicts its own
description.** The row is `Hadfield Manganese Steel`, `hardness_mohs`
8.0, described as "Work-hardens under impact; used in rock crushers and
railroad crossings." A material that work-hardens under impact is
delivered soft and becomes hard at the surface in service. Yet 8.0 is
the third-highest hardness in the file, above `high_speed_steel` at 7.0,
`tool_steel_d2` at 6.5 and `stainless_440c` at 6.0, exceeded only by
`tungsten_carbide` and `titanium_carbide` at 9.0. Those two claims
cannot both describe the same delivered bar. The number appears to
record the work-hardened surface while the description records the
supply condition, which is the same missing-condition problem as the
aluminium tempers, here producing an internally inconsistent row.

**3. `electrum` has a density inconsistent with its own composition.**
The row is `Au:55|Ag:45`, density 15.5. Reading the composition as mass
percentages, as every other row appears to be read, the mixture density
is one divided by the sum of each mass fraction over that element's
density. Using the project's own `data/chemistry/elements.csv` figures
of 19.282 for gold and 10.501 for silver: 0.55 divided by 19.282 is
0.028524, and 0.45 divided by 10.501 is 0.042853; the sum is 0.071377,
and one divided by that is 14.01 g/cm3. The row says 15.5, about 11
percent high. The same calculation on `rose_gold`,
`Au:75|Cu:22.5|Ag:2.5`, gives 15.06 against a listed 15.0, and on
`tumbaga`, `Au:30|Cu:70`, gives 10.67 against a listed 11.0, both within
a few percent. So the method is right and `electrum` is the
outlier. Its listed value is what you would get if that one row's
composition were atom percentages rather than mass percentages. **The
underlying issue is that the `composition` column does not state whether
its numbers are by mass or by atom**, and at least one row only makes
sense the other way.

**4. `carbon_steel` and `mild_steel` carry identical conductivity
values**, both 51.9 thermal and 5.96e6 electrical, despite different
carbon contents. Plausible as rounding, but it reads like one row copied
from the other.

**5. Two conductivity columns have no units in their names**, unlike
`density_g_cm3`, `melting_point_k` and `tensile_strength_mpa`.

**6. The file editorialises about lead in the one row that does not
contain it.** `pewter` is described as "formerly contained lead," while
`solder_60_40` (`Sn:60|Pb:40`), `type_metal` (`Pb:75|Sb:20|Sn:5`) and
`wood_metal` (25 percent lead plus 12.5 percent cadmium) carry no such
note. A reader scanning descriptions would conclude the opposite of the
truth.

**Columns a teaching file about metals arguably needs and lacks:**
`condition` or `temper`, `yield_strength_mpa`, `elastic_modulus_gpa`,
`elongation_pct`, a galvanic series position, and a hazard flag. The
first four are the properties this guide spent its first third
explaining are different from each other, and the file records exactly
one of them.

**One note on the source registry.** The `placeholder-alloy-data` entry
in `data/sources/registry.json` says it stands in for a citation never
made and should be replaced with `mil-hdbk-5` for aluminium and titanium
allowables. There is no `mil-hdbk-5` entry to replace it with. For this
topic `nasa-ntrs` covers those allowables directly, and its own registry
note already names CR-123772 for 6061 and CR-123773 for 7075, both read
for this guide.

## You own this when

- You can say why metal bends at all, in terms of something moving
  through a crystal, and can name three ways of making that movement
  harder.
- You ask which strength somebody means, and know the stiffness answer
  is nearly the same for every steel.
- You look at a sagging shelf and reach for a deeper section rather than
  a stronger alloy.
- You expect parts to fail at connections, notches and threads rather
  than in the middle of a bar, and you round internal corners without
  being told to.
- You treat a scratch on a part that cycles as a defect, not a blemish.
- You know hardening and tempering are two halves of one operation and
  that nobody stops after the first half.
- You will not use a heat-treating recipe without knowing which alloy it
  was written for.
- You check what two metals are before bolting them together in the wet,
  and know which one should be the small one.
- You understand that stainless is a film, that chloride and trapped
  water destroy it, and that the dangerous corrosion leaves no visible
  mess.
- You look up a metal's composition before putting heat to it, and know
  the four elements that make that question urgent: zinc, cadmium, lead
  and beryllium.
- You can read a materials table and immediately ask the two questions
  it usually does not answer: in what condition, and compared with what.

That last habit transfers furthest. Nearly every materials failure in
ordinary life is not a mystery about metal. It is a number that was true
about a different piece of metal than the one in your hand: a different
temper, a different grade, a different temperature, a different
neighbour in the joint, or a different number of cycles ago.

## Sources

Every URL below was opened and read while writing this guide.
Government publications are listed first because they are works of the
United States federal government, are in the public domain under 17 USC
105, and can be redistributed with this guide. Bracketed ids are the
entries in `data/sources/registry.json` that cover them.

### United States government (public domain)

- Department of Energy. *DOE Fundamentals Handbook: Material Science,
  Volume 1 of 2*, DOE-HDBK-1017/1-93, January 1993. [`doe`]
  https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1017-93_VOL1.pdf
  Gave: lattice types, grains and boundaries; point and line defects,
  edge and screw dislocations, the rule that a dislocation cannot end
  inside a crystal, and slip; stress, strain and Young's modulus as
  stress over strain below the proportional limit; Table 1, Properties
  of Common Structural Materials, with modulus, yield and ultimate for
  carbon steel, stainless and aluminium; the 0.2 percent offset method
  and the instruction to state the offset; toughness as the work to
  fracture one cubic inch, by Charpy or Izod on a notched sample; cold
  work and that it decreases ductility; the three purposes of annealing;
  that faster cooling gives smaller grains and a harder metal; that as
  hardness and tensile strength rise in heat-treated steel, toughness
  and ductility fall; the Brinell rule of about 500 times the Brinell
  number below 200,000 psi with the 352 Brinell example; the alloying
  effects of nickel, chromium and copper, including that copper raises
  hardness by retarding dislocation movement; that type 304 cannot
  usefully be hardened by heat treatment; and hydrogen embrittlement by
  grain-boundary diffusion and methane formation.
- Department of Energy. *DOE Fundamentals Handbook: Material Science,
  Volume 2 of 2*, DOE-HDBK-1017/2-93, January 1993. [`doe`]
  https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1017-93_VOL2.pdf
  Gave: brittle cleavage fracture; the nil-ductility transition
  temperature, that it is not precise, varies with prior treatment and
  impurities, is found by drop-weight test, and is lowered by small
  grain size and by small nickel and manganese additions; fatigue
  failure, that its primary cause is not well known, the crack
  initiation and propagation sequence, and that it can be initiated by
  microscopic cracks, notches and even grinding and machining marks;
  that work hardening reduces ductility and raises apparent yield
  stress; and creep, its three stages, and the stainless above 1000
  degrees F and zircaloy above 500 degrees F examples.
- Department of Energy. *DOE Fundamentals Handbook: Chemistry, Volume 1
  of 2*, DOE-HDBK-1015/1-93, February 1993. [`doe`]
  https://www.energy.gov/sites/default/files/2026-04/DOE-HDBK-1015-93_VOL1.pdf
  Gave (Module 2, Corrosion): the definition of galvanic corrosion; that
  the potential difference drives current and corrodes one metal; that
  the active metal is the anode and the noble one is protected; that
  without electrical contact both would be attacked as if the other were
  absent; that a larger potential difference means greater probability;
  cathodic protection and sacrificial anodes with zinc named for
  seawater systems; passivity as a stable tenacious oxide barrier, the
  metals that form it in air or pure water at room temperature, and that
  the film may be invisible; pitting and crevice corrosion, the
  differential aeration cell, the low-flow requirement, the rapid
  penetration with little loss of mass, and the four preventions
  including designing out crevices; and stress corrosion cracking, its
  three required conditions, that general corrosion can be essentially
  nil while it occurs, that 18-8 stainless is susceptible in chloride
  and concentrated caustic but not in nitrate, sulfite or ammonium, and
  that cracking can occur in minutes.
- Departments of the Army and the Air Force. *Welding Theory and
  Application*, TM 9-237 / TO 34W4-1-5, 6 November 1967.
  [`dod-technical-manuals`] Read as the full text at the Internet
  Archive.
  https://archive.org/download/TM9-237/TM9-237_djvu.txt
  Gave (Chapter 2): the property definitions of tensile, shear and
  compressive strength, elasticity, elastic limit, yield point and
  strength, modulus of elasticity, ductility, malleability, toughness,
  hardness and machinability, including that toughness decreases as
  hardness increases; cast iron carbon ranges and that it breaks short;
  the carbon bands for low, medium, high carbon and tool steels with
  their weldability; that low carbon steel, wrought iron and steel
  castings cannot be hardened; the alloying effects of chromium, nickel,
  manganese and molybdenum; that stainless properties are due to a very
  thin surface oxide film; the descriptions of aluminium, copper,
  beryllium copper, nickel copper, high brasses, lead, zinc and
  magnesium including the silver nitrate test; and the appearance,
  fracture, spark and torch tests, including the manganese ambiguity
  where 0.55 percent carbon sparks like 1.60 to 1.90 percent manganese.
  (Section III, heat treatment): the 0.35 percent carbon threshold; the
  warning against heating close to the melting point and that such steel
  usually cannot be restored; annealing and its purposes; normalising at
  about 100 degrees F above the critical range with still-air cooling;
  hardening and the carbon fixed in a highly stressed finely divided
  state; tempering, the rule that rising temperature raises toughness
  and lowers hardness, and the 370 to 750 degrees F range sometimes as
  high as 1,100; and case hardening by pack carburising. (Chapters 4 to
  7): that fumes from welding or cutting brass, lead, zinc and
  galvanised or cadmium plated parts carry poisonous oxides; the thermit
  warning that the mould must be dried and that burns may follow molten
  metal contacting moisture in it; the instruction to grind cadmium back
  to base metal before silver brazing because cadmium oxide from
  overheating is highly toxic; and the heat affected area, its three
  zones, that arc welding hardens it more than gas welding, that greater
  hardness means more cracking, the 0.35 percent carbon threshold, and
  that plain carbon steels at 0.25 percent or less show no noticeable
  change.
- National Aeronautics and Space Administration. Barrett, R.T.,
  *Fastener Design Manual*, NASA Reference Publication 1228, March 1990.
  [`nasa-ntrs`]
  https://ntrs.nasa.gov/api/citations/19900009424/downloads/19900009424.pdf
  Gave: that plating is usually the limiting factor on service
  temperature; hydrogen embrittlement as a problem with most plating
  methods, the requirement to bake afterwards, the cadmium bake at 375
  degrees F for 23 hours within 2 hours of plating, that exceeding a
  plating's safe temperature can regenerate free hydrogen, and that
  internal hydrogen embrittlement causes delayed failures after proof
  testing with no external indication; zinc plating as sacrificial and
  migrating to scratched areas, with a 250 degrees F service limit and
  degradation above 140; that series 400 stainless has only 12 percent
  chromium and will corrode in some environments; galvanic corrosion set
  up by dissimilar metals with an electrolyte such as moisture, the
  36-entry ranking from magnesium to gold, the rule that farther apart
  means greater action, the active and passive entries for 304 and 316
  with the explanation of passivation, the rule that the anode should be
  the larger mass with its consequences for fasteners, and the magnesium
  isolation requirement; stress corrosion failing a ductile part far
  below yield and the rule that higher heat-treating temperature and
  lower ductility increase susceptibility; that a bolt cycled in tension
  normally breaks near the end of the threaded portion as the point of
  maximum stress concentration, and the reduced-shank remedy; the
  preload analysis showing bolt load does not rise significantly until
  external load exceeds preload; that more smaller fasteners give a more
  fatigue-resistant joint and are preferable to a few high-strength
  ones; the grip length rule that threads are never in bearing and the
  washer remedy; the tool accuracy table giving plus or minus 15 to 30
  percent preload accuracy by torque against plus 1 to plus 8 percent by
  bolt stretch; and the counterfeit grade 8.2 boron bolts marked as
  grade 8, their 0.22 against 0.37 percent carbon, and the 500 against
  800 degrees F behaviour.
- National Aeronautics and Space Administration. Muraca, R.F. and
  Whittick, J.S., *Materials Data Handbook: Aluminum Alloy 6061*, 2nd
  edition, NASA CR-123772, May 1972. [`nasa-ntrs`]
  https://ntrs.nasa.gov/api/citations/19720022808/downloads/19720022808.pdf
  Gave: density 2.70 g/cm3; typical mechanical properties for the O and
  T6 tempers, ultimate 18.0 and 45.0 ksi, yield 8.0 and 40.0 ksi,
  elongation in 2 inches 30 and 17 percent, and a single tensile modulus
  of 10.0 x 10^3 ksi; that the alloy is strengthened by precipitation
  hardening and cold work; annealing at 413 degrees C held 2 to 3 hours
  with slow cooling; solution treatment at 516 to 545 degrees C by
  product form with a rapid cold water quench; natural ageing to T4 at
  room temperature for 96 hours and artificial ageing at 171 to 182
  degrees C for 7.5 to 8.5 hours; that aluminium exhibits no
  characteristic colour changes even at temperatures up to the melting
  point and that temperatures must be controlled by measurement rather
  than judged by appearance; that all specimens in the welded study
  failed at the edge of the weld or in the annealed zone areas which do
  not respond to ageing; and that the effect of welding heat did not
  extend more than 1.5 inches, 38.1 mm, from the weld centreline.
- National Aeronautics and Space Administration. Muraca, R.F. and
  Whittick, J.S., *Materials Data Handbook: Aluminum Alloy 7075*, 2nd
  edition, NASA CR-123773, May 1972. [`nasa-ntrs`]
  https://ntrs.nasa.gov/api/citations/19720022809/downloads/19720022809.pdf
  Gave: density 2.80 g/cm3; typical mechanical properties for the O and
  T6/T651 tempers, ultimate 33.0 and 83.0 ksi, yield 15.0 and 73.0 ksi,
  elongation 17 and 11 percent, and a single tensile modulus of 10.4 x
  10^3 ksi; that fusion welding is not recommended; and that the alloy
  is resistant to stress corrosion cracking in the T73 temper.
- National Bureau of Standards, now NIST. Marshall, R.D., Pfrang, E.O.,
  Leyendecker, E.V., Woodward, K.A., et al., *Investigation of the
  Kansas City Hyatt Regency Walkways Collapse*, NBSIR 82-2465, February
  1982. [`nist`]
  https://nvlpubs.nist.gov/nistpubs/Legacy/IR/nbsir82-2465.pdf
  Gave: the 17 July 1981 collapse; 113 dead and 186 injured; that it was
  the most devastating structural collapse in United States history in
  terms of loss of life and injuries; that the most probable cause was
  insufficient load capacity of the box beam and hanger rod connections;
  the two contributing factors of an inadequate original connection
  design and a construction change from continuous rods to two sets that
  essentially doubled the load; that the load at collapse was only 31
  percent of the ultimate capacity expected under the Kansas City
  building code; that the unchanged arrangement would have given about
  60 percent; and that from the day of construction the walkways had
  only minimal capacity to resist their own weight.
- Occupational Safety and Health Administration, US Department of Labor.
  29 CFR 1910.252, Welding, cutting and brazing, general requirements.
  [`osha`]
  https://www.govinfo.gov/content/pkg/CFR-2024-title29-vol5/xml/CFR-2024-title29-vol5-sec1910-252.xml
  Gave: (c)(4) ventilation in confined spaces, that it applies to
  helpers and others in the vicinity, that an outside worker must be
  stationed, and that oxygen shall never be used for ventilation; (c)(6)
  zinc; (c)(7) lead, including metals coated with lead-bearing materials
  such as paint and the outdoor respirator requirement; (c)(8)
  beryllium, requiring local exhaust ventilation and airline respirators
  indoors, outdoors and in confined spaces unless atmospheric testing
  under the most adverse conditions shows otherwise; and (c)(9) cadmium,
  including the outdoor respirator requirement.
- Occupational Safety and Health Administration, US Department of Labor.
  29 CFR 1926.1053, Ladders. [`osha`]
  https://www.govinfo.gov/content/pkg/CFR-2024-title29-vol8/xml/CFR-2024-title29-vol8-sec1926-1053.xml
  Gave: the requirement that portable ladders support at least four
  times the maximum intended load, with 3.3 times for extra-heavy-duty
  type 1A metal or plastic ladders; that the load is applied in a
  downward vertical direction and, for non-self-supporting ladders, at
  75 and a half degrees from the horizontal; that ladders shall not be
  loaded beyond their maximum intended load or rated capacity; that they
  must be inspected for visible defects periodically and after any
  occurrence that could affect safe use; and that portable ladders with
  structural defects including corroded components must be tagged and
  withdrawn from service until repaired.
- Occupational Safety and Health Administration, US Department of Labor.
  29 CFR 1926.1050, Scope, application and definitions for stairways and
  ladders. [`osha`]
  https://www.govinfo.gov/content/pkg/CFR-2024-title29-vol8/xml/CFR-2024-title29-vol8-sec1926-1050.xml
  Gave: the definition of maximum intended load as the total load of all
  employees, equipment, tools, materials, transmitted loads and other
  loads anticipated to be applied to a ladder component at any one time.
- National Institute for Occupational Safety and Health, CDC. *NIOSH
  Pocket Guide to Chemical Hazards*: Zinc oxide.
  [`niosh-pocket-guide`] https://www.cdc.gov/niosh/npg/npgd0675.html
  Gave: NIOSH REL of 5 mg/m3 TWA with a 10 mg/m3 short-term limit for
  fume; OSHA PEL 5 mg/m3 for fume; IDLH 500 mg/m3; inhalation as the
  route; the metal fume fever symptom list; and the respiratory system
  as target organ.
- National Institute for Occupational Safety and Health, CDC. *NIOSH
  Pocket Guide to Chemical Hazards*: Cadmium dust and fume (as Cd).
  [`niosh-pocket-guide`] https://www.cdc.gov/niosh/npg/npgd0087.html
  Gave: OSHA PEL 0.005 mg/m3 under 1910.1027 covering all cadmium
  compounds; the potential occupational carcinogen designation with
  prostate and lung as cancer sites; IDLH 9 mg/m3 as Cd; the symptom
  list; and respiratory system, kidneys, prostate and blood as target
  organs.
- National Institute for Occupational Safety and Health, CDC. *NIOSH
  Pocket Guide to Chemical Hazards*: Lead. [`niosh-pocket-guide`]
  https://www.cdc.gov/niosh/npg/npgd0368.html
  Gave: NIOSH REL and OSHA PEL both 0.050 mg/m3 as an 8-hour TWA; IDLH
  100 mg/m3 as Pb; inhalation, ingestion and skin or eye contact as
  routes; the symptom list; and the target organs.
- National Institute for Occupational Safety and Health, CDC. *NIOSH
  Pocket Guide to Chemical Hazards*: Beryllium and beryllium compounds
  (as Be). [`niosh-pocket-guide`]
  https://www.cdc.gov/niosh/npg/npgd0054.html
  Gave: OSHA PEL 0.0002 mg/m3 TWA with a 0.002 mg/m3 15-minute
  short-term limit; the carcinogen designation with lung cancer as the
  site; IDLH 4 mg/m3 as Be; and the chronic beryllium disease symptom
  list.
- Federal Aviation Administration, US Department of Transportation.
  *Acceptable Methods, Techniques, and Practices: Aircraft Inspection
  and Repair*, Advisory Circular 43.13-1B with Change 1. **No registry
  id covers the FAA yet**; one is needed before this citation can be
  wired, or the three glossary definitions below can be dropped.
  https://www.faa.gov/documentLibrary/media/Advisory_Circular/AC_43.13-1B_w-chg1.pdf
  Gave, from its glossary only: galvanic corrosion as corrosion due to
  the presence of dissimilar metals in contact with each other; fretting
  corrosion as corrosion damage between close-fitting parts allowed to
  rub together, where the rubbing prevents the formation of protective
  oxide films; and filiform corrosion as a thread or filament-like
  corrosion forming on aluminium skins beneath the finish. **Note:**
  only the glossary and contents of this document extracted as readable
  text. The body of Chapter 6, Corrosion, did not, so nothing from the
  chapter body, including its dissimilar-metals table, is cited here.

### Project data read for this guide

- `data/chemistry/alloys.csv`, all 74 rows, for the column-by-column
  reading, the worked temper comparison and the six reported defects.
- `data/chemistry/elements.csv`, for the element densities used in the
  identification section and in the `electrum` density calculation:
  gold 19.282, silver 10.501, copper 8.96, lead 11.342, titanium 4.54.
- `data/locales/silverdale_wa/locale.json`: the setting at the head of
  Dyes Inlet, an arm of Puget Sound on the Kitsap Peninsula.
- `data/locales/silverdale_wa/climate.json`: 1,446 mm annual
  precipitation, 236 mm in January, January mean high 8.1 degrees C and
  mean low 2.0 degrees C.
- `data/sources/registry.json`: the licence status of every authority
  cited, and the `placeholder-alloy-data` note reported above.

### What could not be sourced, and is labelled in the text

- **That steels have a fatigue limit and aluminium alloys do not.** The
  standard engineering position and the reason aircraft structures are
  given finite lives. No federal publication opened for this guide
  states it, so the text records it as convention. What is sourced is
  that NASA's handbooks for both 6061 and 7075 publish S-N curves rather
  than a single safe stress.
- **The dynamic multiplier on a climbing load.** That stepping down onto
  a rung applies more than body weight for an instant is ordinary
  mechanics, but no figure was found in the ladder regulation or
  elsewhere, so the text gives the direction and refuses the number.
- **MIL-STD-889, the Department of Defense standard on dissimilar
  metals, and MIL-HDBK-5, the metallic materials handbook.** These are
  the canonical references for galvanic compatibility and for alloy
  allowables, and both are federal works. Neither could be opened for
  this guide: the EverySpec mirror returned HTTP 403 Forbidden, and
  copies on commercial standards resellers were not used.
  **Uncheckable**, so nothing is cited from either. The galvanic ranking
  used here is NASA's own published list from RP-1228, read in full.
- **The body of FAA AC 43.13-1B Chapter 6, Corrosion.** The document
  downloaded and its glossary extracted as text; the chapter body did
  not, so only the glossary definitions are cited.
- **The full temper-colour sequence for steel.** Straw, brown and purple
  before blue are universally quoted and appear in none of the manuals
  opened for this guide or for [Sharpening](/library#sharpening). Blue appears
  in two of them, and blue is the one that matters.
- **Verdicts on specific metal pairings in specific environments.** The
  galvanic series gives the physics and the area rule gives the design
  principle. Specific verdicts come from corrosion engineering practice
  and from standards that are sold rather than published, so this guide
  states the mechanism rather than issuing verdicts it cannot source.

### A note on what is not cited

Metal suppliers, fastener manufacturers, knife makers and welding
equipment vendors publish an enormous amount of materials information,
much of it accurate. None of them is an authority on an exposure limit,
a load factor or a heat treating temperature, and where a number here
exists only in that literature it has been left out or labelled as
convention. Nothing in this guide rests on a vendor's claim about a
product. ASTM and ASME standards are referred to by designation where a
source referred to them, and are never reproduced: they are copyrighted
works, unlike the federal regulations that cite them.
