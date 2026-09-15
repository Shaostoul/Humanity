# Units and Converting Them

A unit error does not look like an error. That is the whole problem.

If you drop a decimal point you often notice, because the answer
suddenly looks absurd. If you convert the wrong way, or forget to
convert at all, the number that comes out is a perfectly ordinary
number. It has a sensible size. It fits in the box on the form. It
passes every check a human eye makes at a glance. And it is wrong by a
factor of four, or ten, or a thousand.

This is the error that lost a spacecraft at Mars after a nine month
journey, and it is the error that gives a patient a thousand times the
dose their doctor wrote down. Those two failures are the same failure.
In both cases a correct number was handed to somebody who read it as a
different quantity.

The skill this guide teaches is one habit: **write the unit at every
step of the arithmetic and cancel the units like algebra.** Do that and
a wrong conversion produces a nonsense unit before it produces a wrong
answer. The arithmetic catches the mistake for you, which is the only
kind of checking that works reliably when you are tired.

Everything numbered in this guide comes from the National Institute of
Standards and Technology, the United States federal agency that holds
the national measurement standards, or from another federal authority.
NIST publications are works of the US government and are in the public
domain. Where a statement is a working rule rather than something
somebody published, it says so in the text.

A note on how numbers are written here. Because this file has to stay
plain ASCII, temperatures are written "20 C" and "68 F" rather than
with a degree symbol, and the micro prefix is written "u" rather than
with the Greek letter mu. Both are explained where they first matter.

## One warning before the arithmetic

Most of this guide is about getting a shelf the right length or a tank
the right size. One area is not like that.

**Medication doses are where a unit error kills people, and the trap is
a single letter.** The US Food and Drug Administration states plainly
that the abbreviations "mcg" and the mu symbol for microgram can be
mistaken for "mg", milligram, and that doing so creates a thousandfold
overdose. A thousandfold. Not ten percent out, not double.

If you are ever transcribing a dose, for a person or an animal, copy
the unit character by character and read it back. Do not convert it in
your head. Do not abbreviate it yourself. This guide will teach you why
that particular pair of prefixes is so dangerous, but the rule comes
first: do not improvise with a dose.

The FDA's own definition of a medication error, which it takes from the
National Coordinating Council for Medication Error Reporting and
Prevention, turns on one word. A medication error is any **preventable**
event that may lead to inappropriate use or patient harm while the
medicine is in the hands of a provider, a patient or a consumer. Nobody
classifies these as accidents. They are classified as things somebody
could have stopped, and the way you stop the ones in this guide is by
being fussy about units in writing.

## Units are part of the number, not a label on it

Here is the single most useful idea in this document.

**A measurement is not a number with a word after it. It is a number
multiplied by a unit, and the unit obeys the ordinary rules of
algebra.**

"36 inches" means thirty six times one inch, in the same way that "3x"
means three times x. That sounds like pedantry. It is the opposite of
pedantry: it is what makes the checking automatic.

Watch what happens when you convert a shelf width. An inch is exactly
25.4 millimetres, so the ratio 25.4 mm per 1 inch equals one. You are
allowed to multiply anything by one.

```
36 in  x  25.4 mm/in  =  914.4 mm
```

Look at what the units did. The "in" on the top of the first term
cancels the "in" on the bottom of the conversion factor, exactly like
cancelling a common factor in a fraction, and you are left with
millimetres. Millimetres is the answer you wanted, so the arithmetic
was set up right.

Now do it wrong on purpose. Suppose you could not remember whether to
multiply or divide and you divided:

```
36 in  /  (25.4 mm/in)  =  1.417 in-squared per mm
```

The number 1.417 is not obviously silly. If you had written no units,
you would have written 1.4 in a box and moved on. But the units say
"inches squared per millimetre", which is not a length, is not a thing
at all, and could not possibly be the width of a shelf. **The wrong
method announced itself before the wrong number could escape.**

That is the whole technique. Write the unit on every quantity, write
conversion factors as fractions with units top and bottom, cancel, and
check that what survives is the unit you were asked for.

### The same trick on a rate, where it matters more

Single conversions are easy to get right by eye. Rates and compound
units are where people actually go wrong, and where the cancelling
earns its keep.

A pump is rated at 5.0 gallons per minute. How many litres per hour?

There are two conversions here, volume and time, and you can do them in
one line if you write each as a fraction that cancels what you want
gone:

```
5.0 gal   3.785412 L    60 min
------- x ---------- x  ------  =  1135.6 L/h
  min       1 gal          1 h
```

"gal" cancels against "gal". "min" cancels against "min". What is left
is L on top and h on the bottom, which is litres per hour. If you had
accidentally written the time factor upside down as 1 h per 60 min, the
minutes would not have cancelled and you would have been left with litre
hours per minute squared, which is not a thing.

You do not have to be confident about which way round a conversion goes.
You only have to write the units down and let the cancelling tell you.
That is why this habit is worth more than memorising factors.

### Two small conventions that prevent real confusion

NIST publishes style rules for writing quantities, and two of them are
worth adopting because they head off genuine misreadings.

**Put a space between the number and the unit.** Write 30.2 C, not
30.2C. NIST is explicit that the degree Celsius symbol must be preceded
by a space.

**Do not mix units the way customary measurement does.** In inch and
pound units people write a length as 27 feet 5 inches. Metric practice
does not do that. NIST's own example: write 3.45 m, not 3 m 45 cm.
A single number with a single unit is much harder to misread and much
easier to put into a calculation.

## Where units come from, which is not where you think

Most people carry an assumption that metric units are the converted
ones and inches and pounds are the real ones. In United States law it is
the other way round, and has been for a very long time.

NIST states it directly: units based on the inch, the pound and the
gallon were historically derived from the English system and were
subsequently re-defined as multiples of SI units in US law beginning in
1893. The inch is defined as the length corresponding to 2.54
centimetres, exactly. The gallon is defined as the volume corresponding
to 3.785412 litres.

So the inch is not approximately 25.4 mm. The inch **is** 25.4 mm, by
definition, and has been since a 1959 agreement between the
English speaking nations fixed the yard at 0.9144 metre exactly. The
pound is likewise a definition rather than a measurement: NIST prints it
as 0.453 592 37 kilogram, and the mass table below shows a way you can
satisfy yourself that it really is a definition.

This matters practically. It means a whole set of conversions are
exact, with no rounding error at all, and you can carry them as far as
you like. It also means that when two sources disagree about a
customary conversion, the metric definition is the one that settles it.

## The seven base units, and what happened in 2019

Everything measurable is built out of seven base units. Learning the
list takes a minute and it tells you what kind of quantity you are
holding, which is most of the battle.

| Base unit | Symbol | Measures |
|---|---|---|
| second | s | time |
| metre | m | length |
| kilogram | kg | mass |
| ampere | A | electric current |
| kelvin | K | thermodynamic temperature |
| mole | mol | amount of substance |
| candela | cd | luminous intensity |

Everything else is these seven multiplied and divided. A newton is a
kilogram metre per second squared. A watt is a joule per second, and a
joule is a newton metre. A litre is a thousandth of a cubic metre. When
you are unsure whether two quantities are comparable, break them down
to base units and look.

### The kilogram is no longer a lump of metal in France

Until 2019 the kilogram was defined by an object. The International
Prototype Kilogram, nicknamed Le Grand K, was a small polished cylinder
cast in 1879 from platinum and iridium, kept in a triple locked vault
on the outskirts of Paris. A kilogram was, by definition, the mass of
that particular piece of metal.

In November 2018 the international scientific community voted to
redefine it, and the new definition took effect in 2019. Since then all
seven base units are defined by fixing the numerical values of physical
constants:

| Unit | Fixed constant | Value |
|---|---|---|
| second | caesium hyperfine frequency | 9 192 631 770 Hz |
| metre | speed of light in vacuum | 299 792 458 m/s |
| kilogram | Planck constant | 6.626 070 15 x 10^-34 J s |
| ampere | elementary charge | 1.602 176 634 x 10^-19 C |
| kelvin | Boltzmann constant | 1.380 649 x 10^-23 J/K |
| mole | Avogadro constant | 6.022 140 76 x 10^23 per mole |
| candela | luminous efficacy at 540 THz | 683 lm/W |

Why should you care that the kilogram changed from an object to a
constant? Two reasons, and both are ordinary rather than exotic.

**An object drifts and a constant does not.** NIST says the artefact
masses drifted, meaning they changed measurably over time. A standard
that quietly changes is a standard that silently invalidates every
measurement made against it.

**An object exists in one place.** There was one Le Grand K. Everyone
else had a copy of a copy, and every copying step added uncertainty.
NIST puts the scaling problem well: a 1 kg artefact can be compared
against a 1 kg standard to a few parts in a billion, but a milligram
measured against that same 1 kg standard carries relative uncertainties
of a few parts in ten thousand. A definition made of constants can be
realised in any properly equipped laboratory, at any scale, without
anyone flying to Paris.

For your purposes nothing changed: a kilogram is still a kilogram, to
far more digits than you will ever need. What changed is that the
system stopped depending on one fragile thing that could be dropped.

## Prefixes, and the one that kills people

A prefix multiplies a unit by a power of ten. There are twenty four of
them, four having been added in 2022.

| Name | Symbol | Factor | | Name | Symbol | Factor |
|---|---|---|---|---|---|---|
| quetta | Q | 10^30 | | deci | d | 10^-1 |
| ronna | R | 10^27 | | centi | c | 10^-2 |
| yotta | Y | 10^24 | | milli | m | 10^-3 |
| zetta | Z | 10^21 | | micro | u (mu) | 10^-6 |
| exa | E | 10^18 | | nano | n | 10^-9 |
| peta | P | 10^15 | | pico | p | 10^-12 |
| tera | T | 10^12 | | femto | f | 10^-15 |
| giga | G | 10^9 | | atto | a | 10^-18 |
| mega | M | 10^6 | | zepto | z | 10^-21 |
| kilo | k | 10^3 | | yocto | y | 10^-24 |
| hecto | h | 10^2 | | ronto | r | 10^-27 |
| deka | da | 10^1 | | quecto | q | 10^-30 |

Two rules from NIST that catch people out. **You cannot stack
prefixes**: there is no such thing as a millimicrometre, you write
nanometre. And **the kilogram is the only base unit that already
contains a prefix**, which is a historical accident and the reason the
unit of mass with no prefix at all is the gram.

### The difference that is obvious and the difference that is not

Nobody confuses a kilogram with a kilometre. The units are different
words, they measure different things, and the mistake is impossible to
make.

Now compare **mg** and **ug**. Same base unit. Same number of
characters. One letter apart. Handwritten, in a hurry, on a form, by
somebody who is not thinking about prefixes at all. And the factor
between them is one thousand.

The FDA states that the abbreviations for microgram can be mistaken for
the abbreviation for milligram, creating a thousandfold overdose. That
is not a theoretical concern from a style guide. It is a regulator
describing reports it receives.

The same agency lists three more failures of exactly this kind, all of
them about how a number and its unit are written down:

- **The letter U for units, read as a zero.** The FDA records fatal
  tenfold insulin overdoses caused by misreading "1U", meaning one
  unit, as "10".
- **A trailing zero.** A dose of 5 mg written as "5.0 mg" can be read
  as "50 mg". Tenfold.
- **A missing leading zero.** A dose of 0.5 mg written as ".5 mg" can
  be read as "5 mg". Tenfold again.

Notice what all four have in common. None of them is a calculation
error. Every one is a transcription error in which the number survived
and the unit or the decimal point did not. Arithmetic skill offers no
protection at all. Writing carefully does.

The general lesson for everything else you measure: **prefixes that
differ by one letter are the dangerous ones.** When you write a
quantity for somebody else to act on, write the prefix out in full if
there is any doubt, and write the number in a form that cannot lose a
decimal point.

## The conversions you actually need

Some of these are exact by definition and some are rounded
measurements. The difference matters, so NIST marks it: in its own
tables, conversion factors that are exact are printed in bold.

**Exact** means the relationship is a definition and has no error at
all. An inch is 25.4 mm the way a dozen is 12: not because anybody
measured it and got that answer, but because that is what the word was
declared to mean. A rounded factor, by contrast, is somebody's
measurement written to as many digits as they could justify, and writing
more digits than they printed is inventing information.

### Length

| From | To | Multiply by | Exact? |
|---|---|---|---|
| inch | millimetre | 25.4 | exact |
| foot | metre | 0.3048 | exact |
| yard | metre | 0.9144 | exact |
| mile | kilometre | 1.609 344 | exact |
| nautical mile | kilometre | 1.852 | as NIST lists it |

The first four follow from the single 1959 definition of the yard,
which is why they are all exact together. A foot is a third of a yard,
an inch a thirty sixth, a mile 1760 yards. The nautical mile is a
separate unit from a separate tradition and is not part of that family.

### Mass

| From | To | Multiply by | Exact? |
|---|---|---|---|
| pound (avoirdupois) | kilogram | 0.453 592 37 | a definition, see below |
| ounce (avoirdupois) | gram | 28.349 523 125 | a sixteenth of a pound |
| grain | milligram | 64.798 91 | a seven thousandth of a pound |
| short ton (2000 lb) | kilogram | 907.184 74 | exactly 2000 pounds |
| long ton (2240 lb) | kilogram | 1016.046 908 8 | exactly 2240 pounds |

The grain is worth a second look, because it demonstrates a check you
can run on any table.

NIST lists the pound as 0.453 592 37 kg and the grain as 64.798 91 mg.
Divide the first by the second and you get 7000.000, exactly. That is
not a coincidence and it is not a measurement agreeing with another
measurement to seven digits, which essentially never happens. It is two
printings of the same definition: there are exactly seven thousand
grains in a pound. Run the same check on the short ton and you get
exactly 2000.

**When two conversion factors divide into a round number, they are
almost always two faces of one definition. When they divide into
something ragged, at least one of them was measured.** That is worth
being able to tell, because it tells you how many digits you are allowed
to carry.

### Temperature, which is a section of its own below

| From | To | Formula |
|---|---|---|
| degrees Fahrenheit | degrees Celsius | subtract 32, then divide by 1.8 |
| degrees Celsius | kelvin | add 273.15 |

### Force, pressure, energy, power

| From | To | Multiply by |
|---|---|---|
| pound-force | newton | 4.448 222 |
| pound per square inch | pascal | 6894.757 |
| horsepower | watt | 745.6999 |
| calorie (thermochemical) | joule | 4.184 |
| Btu (International Table) | joule | 1055.056 |

These are the values NIST prints, rounded to seven digits. Some of them
are definitions underneath and some are not, and NIST marks which in its
own tables by printing the exact ones in bold. Seven digits is more than
you will ever need for any of these, so the distinction only matters if
you are chasing a discrepancy.

Notice the qualifiers in the left column. NIST does not list "the
calorie", it lists the **thermochemical** calorie, and it does not list
"the Btu", it lists the **International Table** Btu. Those qualifiers
are there because more than one definition of each is in circulation.
When a unit name comes with a qualifier attached, copy the qualifier
too; it is part of the unit.

## Temperature, which behaves differently from everything else

Every conversion so far has been a pure scale factor. Double the
inches, double the millimetres. Temperature is not like that, and this
is the single most common conversion mistake among people who are
otherwise careful.

The Fahrenheit scale has **an offset as well as a scale**. Its zero is
not the same zero as Celsius. That means there are two different
conversions, and which one you need depends on whether your number is a
temperature or a difference between two temperatures.

### A temperature converts one way

NIST gives the formula:

```
temperature in C  =  (temperature in F  -  32)  /  1.8
```

So an oven set to 350 F is (350 - 32) / 1.8 = 176.67 C.

NIST has a rounding rule for exactly this case, and it is a good one:
a temperature given in whole degrees Fahrenheit should be converted to
the nearest 0.5 degree Celsius, because a Celsius degree is about twice
the size of a Fahrenheit degree and rounding to a whole Celsius degree
throws away real precision. So 350 F is 176.5 C. In a domestic oven you
would set 175 or 180, but you should know you are choosing to round,
not being told to.

### A temperature difference converts the other way

If your number is a **difference**, an interval, a delta, then the
offset does not apply. NIST's formula for a temperature interval is
simply:

```
interval in C  =  interval in F  /  1.8
```

with no subtraction of 32 anywhere. And because a Celsius degree and a
kelvin are the same size, an interval in kelvin is the same number as
an interval in Celsius.

### Why this is a real trap, with a worked example

Say you are working out heat loss through a wall, the calculation in
[Insulation and Heat Loss](/library#insulation-and-heat-loss). It is 68 F
inside and 28 F outside. Your insulation figure is in watts per square
metre per degree Celsius, so you need the temperature difference in
Celsius.

The difference in Fahrenheit is 68 - 28 = 40 F.

**The wrong way**, and the way almost everybody does it the first time,
is to treat that 40 as a temperature and run it through the temperature
formula:

```
(40 - 32) / 1.8  =  4.4 C
```

**The right way** is to recognise it as an interval and divide:

```
40 / 1.8  =  22.2 C
```

Check it the long way if you do not believe the rule. Convert each end
separately. 68 F is (68 - 32) / 1.8 = 20.0 C. 28 F is (28 - 32) / 1.8 =
-2.2 C. The difference between 20.0 and -2.2 is 22.2 C. The interval
formula was right.

**The wrong answer is smaller by a factor of exactly five.** A wall
sized on 4.4 C when the real load is 22.2 C is a wall that will not keep
the room warm, and nothing in the arithmetic looks wrong at any point.

This is not a rare edge case. Any figure written "per degree", any
thermostat differential, any allowance for thermal expansion, any
cooking instruction of the form "let it rise 20 degrees" is an interval,
not a temperature.

**The test that settles it:** ask whether the number would still mean
the same thing if everybody agreed to start counting from a different
zero. A difference would. A temperature would not. If it is a
difference, do not subtract 32.

## Volume, which is the worst area in the customary system

Length and mass in US customary units are merely inconvenient. Volume
is genuinely booby trapped, in three separate ways, and a person can
walk into all three in a single afternoon of cooking.

### A fluid ounce is not an ounce

An ounce is a mass. A fluid ounce is a volume. They are different
quantities, they cannot be converted into each other at all, and the
only thing they share is a word.

To get from a volume to a mass you need the density of the specific
substance, which is a third number and is not in either unit. A fluid
ounce of oil and a fluid ounce of honey are the same volume and
different masses. There is no conversion factor between "fl oz" and
"oz" because the question is not answerable without knowing what is in
the container.

A recipe that says "8 ounces of flour" and a recipe that says "8 fluid
ounces of flour" are asking for two different amounts, and which one is
meant depends entirely on the author. If a recipe matters, weigh.

### A US gallon is not an imperial gallon

| Unit | In litres | Made of |
|---|---|---|
| US gallon | 3.785 412 | 128 US fluid ounces |
| Imperial (UK) gallon | 4.546 09, exact | 160 imperial fluid ounces |

The imperial gallon is about 20 percent larger. That is not a subtlety;
that is the difference between filling a tank and overflowing it.

And the trap goes deeper, because it reverses. The imperial **fluid
ounce** is 28.413 06 mL and the US fluid ounce is 29.573 53 mL, so the
imperial fluid ounce is about 4 percent **smaller**. The imperial
gallon is bigger while the imperial fluid ounce is smaller, because the
two systems chop the gallon into different numbers of pieces. Knowing
that one of them is bigger tells you nothing about the other.

The same reversal applies to pints. Working from the gallon figures
above, a US pint is an eighth of 3.785 L, which is 473 mL, and an
imperial pint is an eighth of 4.546 L, which is 568 mL. A recipe or a
fuel figure written in pints is ambiguous unless you know which country
wrote it.

### A cup is three different sizes, all of them official

This one is worth showing in full, because it is the clearest possible
demonstration that a household measure does not have one settled size,
even inside a single country's own standards.

| Authority | A cup is | Why |
|---|---|---|
| NIST, by definition | 236.6 mL | 8 US fluid ounces, which is what a cup is defined as |
| FDA, for nutrition labels | 240 mL | 21 CFR 101.9, the food labelling regulation |
| NIST, practical metric equivalent | 250 mL | the round figure NIST names for everyday use |

All three are correct within their own rules. The spread from 236.6 to
250 is about 6 percent. For a cup of stock nobody cares. For a cup of a
concentrated ingredient, or for a dose, 6 percent may be the whole
margin you had.

The same regulation and the same NIST footnote treat the smaller
household measures the same way. A teaspoon is defined as a sixth of a
fluid ounce, which is 4.93 mL, and is called 5 mL in practice by both
NIST and the FDA. A tablespoon is half a fluid ounce, 14.79 mL, and is
called 15 mL. Those roundings are small and harmless. The cup is the
one where the rounding is large enough to notice.

And a kitchen spoon out of a drawer is not a teaspoon at all. It is
whatever the manufacturer made it. This is exactly why liquid medicine
should be measured with a marked syringe in millilitres and never with
cutlery.

### The rest of the customary volume zoo

For reference, since these turn up on bags and delivery notes:

| From | To | Multiply by |
|---|---|---|
| cubic inch | cubic centimetre | 16.387 064 |
| cubic foot | cubic metre | 0.028 316 85 |
| cubic yard | cubic metre | 0.764 555 |
| acre-foot | cubic metre | 1233.489 |
| US gallon | litre | 3.785 412 |

## Area and volume do not scale the way length does

Almost everybody gets this wrong, and it is the error most likely to
cost you money on a real job.

**Double a linear dimension and you quadruple the area. Double it in
all three directions and you multiply the volume by eight.** Area goes
as the square of the length scale, volume as the cube.

You can verify this against NIST's own tables without taking anybody's
word for it. A foot is 0.3048 metre exactly. Square that: 0.3048 x
0.3048 = 0.092 903 04, and NIST lists a square foot as 0.092 903 04
square metres. Cube it: 0.3048 x 0.3048 x 0.3048 = 0.028 316 85, and
NIST lists a cubic foot as 0.028 316 85 cubic metres. The area factor
really is the length factor squared, and the volume factor really is
the length factor cubed.

**This means an area or a volume conversion factor is never the same
number as the length factor, and reaching for the length factor is the
mistake to watch for.** There are about 3.28 feet in a metre, and that
number is burnt into most people's memory. Use it on an area and your
answer is about 3.3 times too large. Use it on a volume and your answer
is about 10.8 times too large, because you needed 35.3 cubic feet per
cubic metre and you divided by 3.28 instead.

The unit algebra tells you the right way to build the factor. An area
is a length times a length, so the conversion factor is the length
factor **squared**, and you write it squared so that the units cancel
properly:

```
250 sq ft  x  (0.3048 m/ft)^2  =  250 ft^2 x 0.09290304 m^2/ft^2 = 23.2 sq m
```

The ft-squared on top cancels the ft-squared on the bottom and only
square metres survive. If you had used the unsquared factor you would
have been left with "foot metres", one foot and one metre multiplied
together, which is a visible sign that you converted one of the two
dimensions and forgot the other. It is a subtler warning than a wholly
impossible unit, but it is there if you write the units down.

### A worked example with real consequences

You are building a raised bed 1.2 m long, 2.4 m wide and 0.3 m deep.

```
volume  =  1.2 m  x  2.4 m  x  0.3 m  =  0.864 m-cubed
```

Note the units again: metre times metre times metre gives metres cubed,
which is a volume, which is what soil is sold by. Good.

In cubic feet, dividing by 0.028 316 85, that is about 30.5 cubic feet
of soil to buy.

Now three ways somebody might say "make it bigger", and what each
actually costs:

| "Make it..." | New size | Soil needed | Change |
|---|---|---|---|
| half as deep again | 1.2 x 2.4 x 0.45 m | 1.296 m-cubed | 1.5 times |
| twice as long | 2.4 x 2.4 x 0.3 m | 1.728 m-cubed | 2 times |
| twice as long and twice as wide | 2.4 x 4.8 x 0.3 m | 3.456 m-cubed | **4 times** |

The last row is the one that catches people. "Twice as big" in plan
view is four times the soil, four times the delivery, four times the
cost. You ordered two loads and you need four.

### The same thing backwards, on a tank

Run it the other way and the result is just as surprising.

You have a water drum and you want one that holds twice as much, the
same shape, just bigger. How much bigger does it need to be?

Volume goes as the cube of the linear scale, so to double the volume
you scale every dimension by the cube root of 2, which is 1.26. The new
drum is only **26 percent** taller and 26 percent wider. It will not
look twice as big. It will look barely different, and it will hold
twice the water.

This is why you cannot judge a tank's capacity by eye, and why you
should always read the stated volume rather than compare silhouettes.
It is the same reason a scale model at one tenth size has one hundredth
the surface area and one thousandth the volume and mass.

A practical corollary worth carrying: heat loss goes roughly with
surface area while the space you are heating goes with volume, so a
larger building is inherently cheaper to heat per unit of space than a
small one of the same construction. That is geometry, not insulation
quality, and it is why a tiny cabin is harder to keep warm than its size
suggests.

## Significant figures, which is a question of honesty

This section is usually taught as a rule. It is better understood as a
matter of not lying.

When you write a number down, the number of digits you write is itself
a claim. Writing 11.0 m says you know the length to within about a
tenth of a metre. Writing 10.9728 m says you know it to a tenth of a
millimetre. If you did not, you have told somebody something untrue,
and they may act on it.

**A conversion cannot add precision that the original measurement did
not have.** The calculator will give you eight digits because
calculators always do. Seven of them may be fiction.

### NIST's rule, which is simple and better than "count the digits"

NIST gives a procedure, and it is more useful than the version usually
taught in school because it accounts for something real: the same
relative uncertainty produces different numbers of digits depending on
where you land.

> **(i)** If the first significant digit of the converted value is
> greater than or equal to the first significant digit of the original
> value, keep the same number of significant digits as the original.
>
> **(ii)** If the first significant digit of the converted value is
> smaller than the first significant digit of the original, keep one
> more significant digit.

Two of NIST's own worked examples:

**36 feet to metres.** 36 x 0.3048 = 10.9728 m. The converted value
starts with 1, which is smaller than the original's 3, so rule (ii)
applies: keep one more digit than the original's two. The answer is
**11.0 m**, not 10.9728 m and not 11 m.

**8 feet to metres.** 8 x 0.3048 = 2.438400 m. The converted value
starts with 2, smaller than 8, so rule (ii) again: the original had one
significant digit, so keep two. The answer is **2.4 m**.

The reasoning behind the rule is worth understanding, because then you
do not have to remember it.

A measurement of "36 feet" means somewhere between 35.5 and 36.5, which
is about 1.4 percent either way. Now look at your two choices for the
answer. "11 m" means somewhere between 10.5 and 11.5, about 4.5 percent,
which throws away information you actually had. "11.0 m" means about
0.45 percent, which claims a little more than you had. Neither is a
perfect fit, because the digits of a decimal number only come in whole
steps and your real precision sat between two of them.

NIST's rule resolves that by keeping the digit. It is the better default
because a number usually goes on to be used in something else, and a
figure that has quietly lost a digit of real precision does more damage
downstream than one carrying a slightly optimistic last digit. But you
should understand that you are choosing, not obeying physics.

### When rounding is not innocent

NIST adds a caution that is worth reading twice, because it is the point
at which tidiness becomes a hazard:

For most purposes, 10 feet rounds to 3 metres. But if a safety code
requires 10 feet of clearance from electrical lines, then the converted
value **must** be 3.05 metres, and must stay 3.05 metres until somebody
studies whether 3 metres of clearance is actually adequate.

Rounding changed the requirement. The tidy number was not the same
requirement as the untidy one, and nobody had checked whether the
difference mattered.

The mirror image appears in commerce. NIST notes that federal and state
regulations let a packager round a converted net quantity **down**,
specifically so that the label never overstates what is in the package.
The direction of rounding is chosen so that the error, if any, falls in
the customer's favour.

**The general rule to carry away: when you round a converted value, ask
which direction is safe.** Sometimes it is up, sometimes down, and
sometimes, as with the electrical clearance, you are not allowed to
round at all.

### The everyday version of false precision

You measure a room with a tape and get "about 12 feet". You convert and
write 3.6576 m in your notes. Six months later somebody reads that
number and believes you measured the room to the nearest tenth of a
millimetre.

Write 3.7 m. If you needed better than that, go and measure better.

One more trick from NIST for making your precision visible: "1200 m" is
ambiguous, because a reader cannot tell whether the trailing zeros are
measured or are just placeholders. "1.200 km" is not ambiguous; it says
clearly that all four digits are significant. Choosing your prefix so
the number sits between about 0.1 and 1000 is not just tidiness, it lets
you say exactly how much you know.

## Reading a specification

Three conventions on real specifications confuse newcomers, and all
three are about the gap between the number printed and the object in
your hand.

### Tolerance

A tolerance is the amount of wrongness that is officially allowed.

NIST Handbook 44, which governs the scales and pumps and meters used in
trade in the United States, puts the principle plainly: the official
tolerances are the limits of inaccuracy officially permissible, and it
is recognised that errorless performance of mechanical equipment is
unattainable. Tolerances exist to fix the range of inaccuracy within
which equipment will be approved.

The reasoning behind how tight to set them is equally plain and is worth
borrowing for your own work. Tolerance values are set so that the
permissible errors are small enough that neither the buyer nor the
seller suffers serious injury, yet not so small as to make manufacturing
or maintenance costs disproportionately high.

That is the real definition of a good tolerance: **tight enough that
nobody is hurt, loose enough that the thing can actually be made.** A
specification with no tolerance on it has not been thought about.

Handbook 44 also distinguishes two tolerances for the same device. An
**acceptance tolerance** applies to equipment that is new or newly
reconditioned, and is usually half of the **maintenance tolerance**,
which applies to equipment already in service. The gap between them is
deliberate: it allows a limited amount of deterioration in use before
the device must be rejected or rebuilt. A scale that is fine on a
maintenance test might have failed as a new scale.

And one rule for when you are the one adjusting something: tolerances
are criteria for the inspector, not targets for the technician. When
equipment is being adjusted, the objective should be to adjust as
closely as practicable to zero error. Aim at the middle, not the edge of
the allowance, because everything drifts afterwards.

### Nominal

A nominal size is a name, not a measurement.

The clearest example anywhere is lumber. The American Softwood Lumber
Standard, published by NIST as Voluntary Product Standard PS 20, defines
it: the nominal size is the label designation for a lumber size
category, which does not reflect the dressed size. The nominal size is
greater than the dressed size. A dry two by four is surfaced to 38.1 mm
by 88.9 mm, which is 1 1/2 by 3 1/2 inches.

**A two by four is not two inches by four inches, and the reason is
real, not a swindle.** The board was sawn at roughly two by four when it
was green and rough. Then it dried, and drying shrinks wood across the
grain, as [Working with Wood](/library#working-with-wood) explains.
Then it was planed smooth on all four faces, which removes more. The
nominal size remembers what came off the saw; the dressed size is what
survives to the shelf.

The standard even keeps two dressed sizes, because the shrinkage is
still happening. A nominal two inch thickness is 38 mm (1 1/2 in) when
sold dry, meaning at 19 percent moisture content or below, but 40 mm
(1 9/16 in) when sold green, above that moisture content. The green
board is fatter because it has not finished shrinking yet.

So when you read any nominal figure, on lumber or pipe or anything else,
the question is always: **what is the actual dimension, and where is it
written down?** Design to the dressed size, always.

### Gauge numbers, which run backwards

Wire gauge is the convention that surprises everybody: a bigger number
means a smaller wire.

The National Bureau of Standards, NIST's predecessor, explains why in
its Copper Wire Tables. The American Wire Gauge numbers are
retrogressive, a larger number denoting a smaller wire, corresponding to
the operations of drawing. Wire is made by pulling it through a series
of progressively smaller dies, and the gauge number counts how many
pulls it took. More pulls, thinner wire, higher number.

The system is a geometric progression, not an arbitrary list. Two
diameters are fixed by definition: No. 0000 is 0.4600 inch and No. 36 is
0.0050 inch, with 38 sizes in between. That makes the ratio between any
size and the next larger gauge number 1.122 932 2, and the ratio over
six gauge numbers 2.0050.

Two rules of thumb fall straight out of those figures, and they are
genuinely useful in the field:

- **Six gauge numbers doubles or halves the diameter**, because the
  sixth power of the step ratio is 2.005, which is near enough to 2.
- **Three gauge numbers doubles or halves the cross sectional area**,
  because area goes as the square of the diameter and the cube of that
  squared ratio is the same 2.005. This is the more useful of the two in
  electrical work, because what a wire does electrically follows its
  cross section rather than its diameter, which is exactly why the
  Copper Wire Tables tabulate resistance against gauge number.

Sheet metal and needle gauges use different and mutually incompatible
gauge systems. The only safe habit is to convert a gauge number to an
actual dimension from the table for that specific gauge system before
you use it in any calculation.

## The Mars Climate Orbiter

This is the case that makes the argument, and it is worth getting right
rather than repeating the folk version.

The Mars Climate Orbiter launched on 11 December 1998. Its carrier
signal was last received at about 09:04:52 UTC on Thursday, 23 September
1999, as it passed behind Mars for orbit insertion. It never came out.

NASA's Mishap Investigation Board found the root cause, and stated it in
one sentence: failure to use metric units in the coding of a ground
software file, Small Forces, used in trajectory models.

The detail is the instructive part. Thruster performance data in English
units instead of metric units was used in a software application called
SM_FORCES. Its output went into a file called Angular Momentum
Desaturation. That file was **required by the software interface
documentation to be in metric units**, and the trajectory modellers
assumed it was, because the requirement said so.

The quantity being passed was impulse, which is a force multiplied by a
time. In metric that is newton seconds; the English unit for the same
quantity is the pound-force second. The board's report says only that
the data was in English rather than metric units, but the size of the
error follows from which units those are. NIST gives one pound-force as
4.448 222 newtons, so one pound-force second is 4.448 222 newton
seconds. **A number written in pound-force seconds and read as newton
seconds understates the real impulse to about 22 percent of its true
value**, which is just the arithmetic of dividing by 4.448.

So every one of these small firings was modelled at less than a quarter
of its actual effect. And there were far more firings than anyone
expected: the board found the desaturation events happened 10 to 14
times more often than the navigation team anticipated, because the
spacecraft's solar array was asymmetric and solar pressure kept spinning
it up.

Nine months of small, consistent, invisible errors accumulated. In the
board's words, at the time of Mars insertion the spacecraft trajectory
was approximately 170 kilometres lower than planned. NASA's space
science data archive describes the same moment slightly differently: the
spacecraft entered the Martian atmosphere at about 57 km, against an
intended 140 to 150 km. The two NASA figures do not reconcile exactly,
which is itself worth noticing, because even primary sources round and
paraphrase; the board's report is the more authoritative. Either way the
spacecraft was far too low, and it either burned up or skipped back out
into orbit around the Sun.

The same archive records what it cost: 193.1 million dollars of
spacecraft development, 91.7 million to launch it, and 42.8 million in
mission operations. That is 327.6 million dollars, and nine months, for
a number that was written correctly and read as something else.

**The part everybody misses.** The board did not stop at the units. It
listed eight contributing causes, and the pattern in them is the real
lesson. Among them: the navigation team was unfamiliar with the
spacecraft; communications between project elements were inadequate;
training was inadequate; and the verification and validation process did
not adequately address ground software.

The board's own summary of the situation is the sentence to remember:
sufficient processes are usually in place on projects to catch these
mistakes before they become critical to mission success, but for this
mission the root cause was not caught by the processes in place.

JPL's director said the same thing on the day, and it is the more
quotable version. The problem was not the mistake. It was the inability
to recognise and correct a simple error, and the failure of the systems
engineering and of the checks and balances that should have found it.

**A unit error is normal. Not catching it is the failure.** Somebody
will always write a number in the wrong units; that is a human thing and
it will keep happening. What separates a near miss from a lost
spacecraft is whether anything downstream was set up to notice. Carrying
units through your arithmetic is that check, done by you, for free, at
the moment the mistake is made.

## The conversions worth memorising

Short list. These are the ones that come up constantly. Exactness is
marked because it tells you whether you may carry more digits.

| Quantity | Conversion | Exact? |
|---|---|---|
| Length | 1 in = 25.4 mm | exact |
| Length | 1 ft = 0.3048 m | exact |
| Length | 1 yd = 0.9144 m | exact |
| Length | 1 mile = 1.609 344 km | exact |
| Area | 1 sq ft = 0.092 903 04 sq m | exact (the length factor squared) |
| Volume | 1 cu ft = 0.028 316 85 cu m | exact (the length factor cubed) |
| Volume | 1 US gallon = 3.785 412 L | a definition, see note below |
| Volume | 1 imperial gallon = 4.546 09 L | exact |
| Volume | 1 US fl oz = 29.573 53 mL | follows from the gallon |
| Volume | 1 imperial fl oz = 28.413 06 mL | follows from the imperial gallon |
| Volume | 1 US cup = 236.6 mL by definition, 240 mL on a US nutrition label, 250 mL as NIST's practical metric equivalent | see the volume section |
| Mass | 1 lb = 0.453 592 37 kg | a definition |
| Mass | 1 oz = 28.349 523 125 g | a sixteenth of a pound |
| Temperature | C = (F - 32) / 1.8 | exact |
| Temperature interval | interval in C = interval in F / 1.8 | exact, and no offset |
| Temperature | K = C + 273.15 | exact |
| Force | 1 lbf = 4.448 222 N | as NIST prints it |
| Pressure | 1 psi = 6894.757 Pa | as NIST prints it |
| Speed | 1 mph = 1.609 344 km/h | exact |
| Prefixes | 1 mg = 1000 ug | exact, and the most dangerous line in this table |

**The note on the gallon**, because it shows how a published table can
hide a rounding. NIST states that the gallon is defined as the volume
corresponding to 3.785412 litres. But a US gallon is also defined as
exactly 231 cubic inches, and an inch is exactly 25.4 mm, so you can
work the exact figure out yourself: 231 times 16.387064 cubic
centimetres gives 3785.411784 mL, which is 3.785 411 784 L. The 3.785412
in the table is that exact number rounded to seven digits. Both are
NIST's, both are right, and the difference will never matter to you.
It is a reminder that "the published value" and "the exact value" are
not always the same thing, and that the way to tell is to rebuild the
number from its definition.

Two rough approximations for mental arithmetic, useful for sanity
checking and never for a specification. These are **rules of thumb**,
not sourced values, and the exact figures above are what you calculate
with:

- A metre is a bit over a yard, so to get metres, divide feet by about
  three and a quarter.
- A kilogram is a bit over two pounds, so to get pounds, double the
  kilograms.

Use them to notice that an answer is ten times out. Never use them to
cut anything, and never use them on a dose.

### One historical footnote you may meet

If you work with old survey data or land records in the United States,
you may meet the **US survey foot**. In 1893 the US foot was legally
defined as 1200/3937 metre. In 1959 it was refined to 0.3048 metre to
match other countries, but existing geodetic survey data was allowed to
keep the old standard under the name US survey foot. NIST states the new
foot is shorter by about two parts in a million. That is around three
millimetres in a mile, which is arithmetic on NIST's ratio rather than a
figure NIST prints, and it is nothing on a shelf and a great deal on a
property boundary.

NIST deprecated the US survey foot after 31 December 2022. From 1
January 2023 it is obsolete and superseded by the international foot,
and should be avoided except for historic and legacy work. If you are
reading old survey plans, check which foot they used.

## You own this when

- You write the unit on every quantity in a calculation, and you check
  that the units that survive the cancelling are the units you were
  asked for.
- You can say which conversions are exact by definition and which are
  rounded measurements, and you know that the inch, the foot, the yard,
  the pound and the gallon are all defined from metric units rather
  than the other way round.
- You never write mg when you mean ug, and you never abbreviate
  somebody else's dose.
- You convert a temperature difference by dividing by 1.8 and a
  temperature by subtracting 32 first, and you can say which one you are
  holding without thinking about it.
- You know that "twice as big" in two dimensions is four times the
  material and in three dimensions is eight times, and you check which
  one somebody meant before ordering.
- The number of digits you write down is a number of digits you could
  defend, and when you round a converted value you have asked which
  direction is the safe one.
- You read a spec and ask three questions: what is the tolerance, is
  that dimension nominal or actual, and if it is a gauge number, what is
  the real dimension in the table.

When those are true, the arithmetic is checking itself. That is the
point of all of it. You are not being asked to be more careful than a
human being can be; you are being asked to write the units down so the
calculation can be careful on your behalf.

## Sources

Grouped by what kind of authority each one is. Every source here is a
work of the United States federal government and is therefore in the
public domain, so all of it can be redistributed with this guide. That
is unusual and it is why this topic could be written entirely from
primary authorities: NIST is the national measurement institute, and
nearly everything in this guide is its job.

### United States government (public domain)

- National Institute of Standards and Technology. *The International
  System of Units (SI): Conversion Factors for General Use*, NIST
  Special Publication 1038, 2006 (the inch defined as exactly 2.54 cm
  and the gallon as 3.785412 L; customary units re-defined as multiples
  of SI in US law from 1893; the pound as 0.45359237 kg; the full
  conversion tables for length, area, volume, mass, velocity, force and
  energy; the cup, tablespoon and teaspoon defined as 8, 1/2 and 1/6
  fluid ounces with practical metric equivalents of 250 mL, 15 mL and
  5 mL; the gallon as 128 fluid ounces or 231 cubic inches; the
  temperature conversion formula and, separately, the temperature
  interval formula; the significant digit rules for rounding converted
  values; the 10 feet to 3 metres electrical clearance caution; the
  packaging round-down allowance; the 0.5 degree Celsius temperature
  rounding rule; the 1959 refinement of the foot and the US survey
  foot's two parts in a million difference; the note that metric
  practice writes 3.45 m rather than 3 m 45 cm).
  https://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication1038.pdf
- National Institute of Standards and Technology. *Guide for the Use of
  the International System of Units (SI)*, NIST Special Publication 811,
  Appendix B, Conversion Factors (the convention that exact factors are
  printed in boldface; the exact factors for inch, foot, yard, mile,
  square foot and imperial gallon; the imperial fluid ounce; the
  pound-force, psi, horsepower, calorie and Btu factors; section B.7 on
  rounding numbers and rounding converted numerical values, including
  the 36 feet to 11.0 m worked example; section B.3 on using a
  conversion factor).
  https://www.nist.gov/pml/special-publication-811/nist-guide-si-appendix-b-conversion-factors
  and https://www.nist.gov/pml/special-publication-811/nist-guide-si-appendix-b-conversion-factors/nist-guide-si-appendix-b9
- National Institute of Standards and Technology. *Guide for the Use of
  the International System of Units (SI)*, NIST Special Publication 811,
  Chapter 7, Rules and style conventions for expressing values of
  quantities (the space between number and unit symbol; the degree
  Celsius spacing rule; no compound prefixes; the kilogram as the only
  base unit containing a prefix; using the prefix to make significant
  digits unambiguous, 1.200 km against 1200 m).
  https://www.nist.gov/pml/special-publication-811/nist-guide-si-chapter-7-rules-and-style-conventions-expressing-values
- National Institute of Standards and Technology. Definitions of the SI
  base units (all seven base units and the fixed numerical values of the
  seven defining constants, as adopted in 2019).
  https://www.nist.gov/si-redefinition/definitions-si-base-units
- National Institute of Standards and Technology. The kilogram (Le Grand
  K as a platinum-iridium cylinder cast in 1879 and kept in a vault near
  Paris; artefact drift; the scaling problem from 1 kg to 1 mg; the
  November 2018 vote and the 2019 effective date).
  https://www.nist.gov/si-redefinition/kilogram
- National Institute of Standards and Technology. Metric (SI) prefixes
  (all 24 prefixes with symbols and factors, and the four added in
  2022). https://www.nist.gov/pml/owm/metric-si-prefixes
- National Institute of Standards and Technology. SI units, length (the
  inch as exactly 25.4 mm, derived from the yard as fixed on 1 July
  1959). https://www.nist.gov/pml/owm/si-units-length
- National Institute of Standards and Technology. U.S. Survey Foot,
  revised unit conversion factors (the 1200/3937 definition, the
  0.3048 m international foot, and the deprecation of the survey foot
  after 31 December 2022). https://www.nist.gov/pml/us-surveyfoot
- National Institute of Standards and Technology. *Specifications,
  Tolerances, and Other Technical Requirements for Weighing and
  Measuring Devices*, NIST Handbook 44, Appendix A, Fundamental
  Considerations, sections 2.1 to 2.3 (tolerances as the limits of
  inaccuracy officially permissible; errorless performance being
  unattainable; acceptance tolerances usually half of maintenance
  tolerances and why; the theory that tolerances must not injure buyer
  or seller yet must not make equipment disproportionately costly; and
  the instruction to adjust as closely as practicable to zero error).
  https://www.nist.gov/system/files/documents/2025/12/30/appa-26-HB44-20251209.pdf
- National Institute of Standards and Technology, US Department of
  Commerce. *American Softwood Lumber Standard*, Voluntary Product
  Standard PS 20-20 (the definition of nominal size; a dry two by four
  surfaced to 38.1 mm by 88.9 mm; the definitions of dry and green
  lumber at 19 percent moisture content; Table 3 nominal and
  minimum-dressed sizes, dry and green).
  https://www.nist.gov/system/files/documents/2019/12/11/PS%2020-20%20final%20WERB%20approved.pdf
- United States National Bureau of Standards, now NIST. *Copper Wire
  Tables*, NBS Circular 31, 4th edition (the American Wire Gauge numbers
  being retrogressive and corresponding to drawing operations; No. 0000
  at 0.4600 inch and No. 36 at 0.0050 inch with 38 sizes between; the
  step ratio of 1.1229322 and the sixth-power ratio of 2.0050).
  https://nvlpubs.nist.gov/nistpubs/Legacy/circ/nbscircular31e4.pdf
- National Aeronautics and Space Administration. Mars Climate Orbiter
  Mishap Investigation Board, Phase I Report, 10 November 1999,
  reproduced in full within the Board's Phase II report hosted by NASA
  Langley (the root cause statement; the SM_FORCES software and the
  Angular Momentum Desaturation file; English units where the interface
  documentation required metric; desaturation events occurring 10 to 14
  times more often than expected; the trajectory approximately 170 km
  lower than planned at Mars insertion; the loss of carrier signal at
  09:04:52 UTC on 23 September 1999; the eight contributing causes; and
  the Board's observation that processes are usually in place to catch
  such mistakes).
  https://discovery.larc.nasa.gov/PDF_FILES/mars_climate_orbiter_phaseII.pdf
- National Aeronautics and Space Administration, Goddard Space Flight
  Center. NASA Space Science Data Coordinated Archive, Mars Climate
  Orbiter, 1998-073A (the 23 September 1999 loss date; the intended 140
  to 150 km altitude against an actual entry at about 57 km; spacecraft
  development, launch and operations costs).
  https://nssdc.gsfc.nasa.gov/nmc/spacecraft/display.action?id=1998-073A
- National Aeronautics and Space Administration. Mars Climate Orbiter
  mission page (launch on 11 December 1998; the loss nine months later;
  ground software in English units against onboard software in metric).
  https://science.nasa.gov/mission/mars-climate-orbiter/
- NASA Jet Propulsion Laboratory. Mars Climate Orbiter Team Finds Likely
  Cause of Loss, 30 September 1999 (the spacecraft team in Colorado and
  the navigation team in California; the statement that the failure was
  of systems engineering and of the checks and balances in the process,
  not only of the individual error).
  https://www.jpl.nasa.gov/news/mars-climate-orbiter-team-finds-likely-cause-of-loss/
- United States Food and Drug Administration. A Microgram of Prevention
  is Worth a Milligram of Cure: Preventing Medication Errors in Animals
  (the microgram abbreviations being mistaken for milligram and creating
  a thousandfold overdose; fatal tenfold insulin overdoses from
  misreading the abbreviation U for units; trailing zeros and missing
  leading zeros each producing tenfold overdoses).
  https://www.fda.gov/animal-veterinary/resources-you/microgram-prevention-worth-milligram-cure-preventing-medication-errors-animals
- United States Food and Drug Administration. Medication Errors Related
  to CDER-Regulated Drug Products (the definition of a medication error
  used by the agency).
  https://www.fda.gov/drugs/drug-safety-and-availability/medication-errors-related-cder-regulated-drug-products
- United States Food and Drug Administration. 21 CFR 101.9, Nutrition
  labeling of food, paragraph (b)(7)(viii) (a teaspoon means 5 mL, a
  tablespoon 15 mL, a cup 240 mL and 1 fl oz 30 mL for labelling
  purposes; servings expressed in household measures followed by the
  metric equivalent).
  https://www.govinfo.gov/content/pkg/CFR-2024-title21-vol2/xml/CFR-2024-title21-vol2-sec101-9.xml

### Inside this project

- [Insulation and Heat Loss](/library#insulation-and-heat-loss), where the
  temperature interval trap in this guide does real damage if you get it
  wrong.
- [Working with Wood](/library#working-with-wood), for why a board
  shrinks between the saw and the shelf, which is the reason nominal
  lumber sizes exist at all.

### Labelled in the text as rules of thumb, not sourced

- The two mental-arithmetic approximations at the end of the reference
  table, that a metre is a bit over a yard and a kilogram a bit over two
  pounds. Use them to catch an answer that is ten times out, never to
  cut or dose anything.
- The two American Wire Gauge shortcuts, that six gauge numbers doubles
  the diameter and three gauge numbers doubles the cross sectional area.
  The underlying ratios are from the Bureau of Standards; the shortcuts
  are arithmetic on those ratios, and they are approximate because the
  true sixth-power ratio is 2.0050 rather than 2.
- The figure of about three millimetres per mile for the difference
  between the survey foot and the international foot. NIST publishes the
  ratio, two parts in a million; the per-mile figure is arithmetic done
  here.
- The observation that heat loss follows surface area while heated space
  follows volume, so larger buildings are cheaper to heat per unit of
  space. The geometry is exact; the application is a general principle
  rather than a measured claim, and real buildings differ in
  construction as well as size.
