# The Climate Where You Live

Find a straight-sided container. A tin can with the label off, a
cylindrical food tub, anything whose walls do not taper. Set it on flat
open ground away from the house and the trees, and leave it there until
the next rain stops. Then stand a ruler in it and read the depth.

That number is real. You measured it. It is also almost useless on its
own, and understanding exactly why is most of what this document is
about.

Say it read half an inch. Is that a lot? You cannot answer. Half an inch
in July at Silverdale would be a substantial fraction of the whole
month's rain. Half an inch in December would be an unremarkable Tuesday.
The same measurement means opposite things depending on a context you do
not have yet. That context is climate: not what happened, but what
usually happens, how much it varies, and how far the unusual can go.

**The gap between weather and climate is the gap between one reading and
a distribution.** Your can holds weather. A thirty-year record holds
climate. This document is about reading the second one honestly, because
most people who look up their local climate come away with a single
number, and a single number is the one form in which climate information
is reliably misleading.

## What this document will and will not do

It will not tell you what the weather will be. Nothing can, past about a
week, and a climate record least of all. If you finish this expecting to
predict next April, the document has failed.

It will not replace your own observations. The nearest official weather
station to Silverdale is over five miles away, at a different elevation,
a different distance from the water. Its record is the best starting
point you have and it is not a description of your yard.

**What it will do is teach you to read a climate record and know what
each number can carry.** By the end you should be able to answer four
questions about your own place: what is normal, how much does it vary,
what is the extreme I have to survive rather than plan around, and where
do I go to look my own numbers up.

Those are four different questions with four different answers, and they
come from four different fields in the record. Confusing them is the
standard failure. A gardener who plants to the average loses the crop.
A builder who sizes a gutter to the average floods the foundation. A
person who sizes a heating system to the record low spends twice what
they needed to.

Everything specific here is Silverdale, Kitsap County, Washington, on
Dyes Inlet, latitude about 47.65 N. The method is general. Where a fact
is local, it says so.

## The record this is built on

HumanityOS ships a climate record for Silverdale at
`data/locales/silverdale_wa/climate.json`. Open it. Everything below is
either in that file or in the federal sources the file cites, and the
whole point of the exercise is that you could have checked it yourself.

**The record is not from Silverdale.** It is from a cooperative
observer station in Bremerton, NOAA identifier USC00450872, at 47.5689
N, 122.6828 W, elevation 33.5 m. That is about 8.5 km, or 5.3 miles,
south of the centre of Silverdale.

Why not closer? Because roughly fifteen stations sit nearer, and every
one of them is a volunteer precipitation gauge with no thermometer. If
you want temperature and frost dates from a station with a complete
1991-2020 record, Bremerton is the nearest one there is. That is a
constraint, not a choice, and it is the first honest thing the record
tells you.

Here is the monthly table, converted to the units the file stores.

| Month | Mean high | Mean low | Precipitation | Daylight |
|---|---|---|---|---|
| January | 8.1 C (46.6 F) | 2.0 C (35.6 F) | 236 mm (9.28 in) | 8.9 h |
| February | 9.7 C (49.4 F) | 1.7 C (35.1 F) | 148 mm (5.83 in) | 10.3 h |
| March | 12.1 C (53.7 F) | 3.2 C (37.7 F) | 162 mm (6.37 in) | 11.9 h |
| April | 15.0 C (59.0 F) | 5.0 C (41.0 F) | 98 mm (3.86 in) | 13.7 h |
| May | 18.8 C (65.8 F) | 8.1 C (46.5 F) | 60 mm (2.36 in) | 15.2 h |
| June | 21.2 C (70.2 F) | 10.6 C (51.1 F) | 41 mm (1.61 in) | 16.0 h |
| July | 24.7 C (76.5 F) | 12.6 C (54.6 F) | 21 mm (0.83 in) | 15.6 h |
| August | 25.2 C (77.3 F) | 12.8 C (55.1 F) | 28 mm (1.09 in) | 14.3 h |
| September | 22.2 C (71.9 F) | 10.6 C (51.1 F) | 46 mm (1.80 in) | 12.6 h |
| October | 15.9 C (60.6 F) | 7.1 C (44.7 F) | 131 mm (5.14 in) | 10.9 h |
| November | 10.7 C (51.2 F) | 3.7 C (38.7 F) | 225 mm (8.84 in) | 9.3 h |
| December | 7.6 C (45.6 F) | 1.7 C (35.0 F) | 252 mm (9.92 in) | 8.4 h |

The Celsius and millimetre values are the ones our file stores. The
Fahrenheit and inch values are NOAA's own published figures for the
station, not conversions of the rounded Celsius, which is why a few
differ by a tenth from what you would get by converting the left-hand
column yourself. That is not sloppiness, and the last section but one
explains exactly where such a tenth comes from.

**Read the shape before you read any single row.** Temperature swings
from a December mean of 4.6 C to an August mean of 19.0 C, a range of
about 14 C across the year. That is a small annual range; places far
from an ocean swing three times as much. Precipitation, by contrast,
swings from 21 mm in July to 252 mm in December, a factor of twelve.

That is the signature of this climate in one sentence: **the temperature
barely moves and the water moves enormously.** Everything practical
follows from it. You do not design here against cold. You design against
water in winter and against drought in summer, in the same year, on the
same piece of ground.

The classification in the file is Koppen Csb, warm-summer Mediterranean,
which surprises people who associate the label with Greece. The Koppen
test for the "s" is a dry summer, and the summer here is genuinely dry:
the driest summer month is under 40 mm and is under a third of the
wettest winter month. Silverdale passes that test by a wide margin, at
21 mm against 252 mm.

## What a normal actually is

The numbers in that table are not last year's weather. They are not a
long-run average of all records. They are **climate normals**, a
specific product with a specific definition, and NOAA's National Centers
for Environmental Information states it plainly: "A 'normal' is the
30-year average of a particular variable's measurements, calculated for
a uniform time period."

Three things in that sentence matter.

**Thirty years, not all years.** The current normals cover 1991-2020.
Not 1895 to now. Not the last decade. A fixed, stated, thirty-year
window.

**A uniform period, so places can be compared.** Every station's normal
covers the same thirty years, which is what lets you compare Bremerton
to Boise without one of them being computed over a different stretch of
history.

**Recomputed on a schedule.** NCEI says it "generates the official U.S.
normals every 10 years in keeping with the needs of our user community
and the requirements of the World Meteorological Organization (WMO) and
National Weather Service (NWS)." The WMO requires member nations to
compute thirty-year averages at least every thirty years and recommends
an update each decade. The 1991-2020 set was first released in May 2021.
The next set will be 2001-2030, and it will move these numbers.

So when you read "the average December rainfall in Silverdale," the
honest full sentence is: the arithmetic mean of December totals measured
at a station in Bremerton over the thirty Decembers from 1991 to 2020,
published in 2021, and due to be replaced.

### Why the average of thirty Decembers describes no December

This is the part that does real damage when it is missed.

Take the December normal, 252 mm. It is a true statement about a set of
thirty numbers. It is not a statement about any one of them. In all
likelihood no single December in 1991-2020 delivered 252 mm; the average
of a set need not be a member of the set, and for a continuous quantity
it almost never is.

That is not a technicality. **It means the normal is a summary of a
distribution, and a summary throws away the thing you usually need.**
The gardener does not need to know the mean last-frost date. The
gardener needs to know how late the frost can come. The builder does not
need the mean December rainfall. The builder needs to know how much
falls in the worst December in a working lifetime. Neither question is
answered by the average, and both are answered by other fields in the
same record, which is why it is worth learning to look past the headline
number.

**A normal is also not a forecast.** It carries no information about
next year beyond the weak claim that next year will probably be drawn
from something like the same distribution. NOAA publishes normals as a
baseline for comparison, so that a given month can be called wetter or
drier than usual, and as an input to engineering and agricultural
decisions. It does not publish them as a prediction, and the word
"normal" is doing a lot of unhelpful work in ordinary English. In a
climate record it means "the thirty-year average," nothing more. It does
not mean typical, expected, or deserved.

## Average, median, and the shape of rain

Here is where a climate record earns its keep, and where our own file
has to hand off to the source.

Our file carries one precipitation number per month: the mean. NOAA's
monthly normals record for the same station carries eighteen
precipitation fields, including the 25th, 50th and 75th percentiles.
The 50th percentile is the median: the value with half the years above
and half below.

Compare them. These are all from the NOAA monthly normals file for
station USC00450872, in inches.

| Month | Mean | Median | 25th pctl | 75th pctl |
|---|---|---|---|---|
| January | 9.28 | 7.93 | 6.22 | 12.65 |
| February | 5.83 | 5.95 | 2.94 | 7.03 |
| March | 6.37 | 5.29 | 4.39 | 8.33 |
| April | 3.86 | 3.68 | 2.39 | 5.09 |
| May | 2.36 | 2.24 | 1.11 | 3.63 |
| June | 1.61 | 1.58 | 0.90 | 2.07 |
| July | 0.83 | 0.76 | 0.15 | 1.27 |
| August | 1.09 | 0.75 | 0.23 | 1.46 |
| September | 1.80 | 1.48 | 0.61 | 2.31 |
| October | 5.14 | 4.44 | 3.73 | 6.25 |
| November | 8.84 | 8.31 | 5.29 | 11.99 |
| December | 9.92 | 10.11 | 6.11 | 12.73 |

**In ten months of the twelve, the mean is higher than the median.**
That is the fingerprint of a right-skewed distribution: most years
cluster somewhat below the average, and a few very wet years drag the
average up above where most of the years actually sit.

August is the clearest case. The mean August is 1.09 inches. The median
August is 0.75 inches. The mean is 45 percent higher than the median.
If you planned your summer water around "an inch of rain in August," you
planned around a number that more than half of Augusts fail to reach.

January is the largest absolute gap: mean 9.28, median 7.93, a
difference of 1.35 inches. More than half of Januaries are drier than
the January you would read off the headline table.

**Two months run the other way.** February's median (5.95) sits above
its mean (5.83), and December's median (10.11) above its mean (9.92).
Those months are left-skewed in this record: a few unusually dry ones
pull the average down. Skew is not a universal law about rain; it is a
property of a particular month at a particular station, and the record
will tell you which way it goes if you look.

### The mode, and why nobody quotes it

The mode is the most common value. For monthly rainfall it is close to
meaningless, because monthly totals are continuous and no two years land
on the same number, so every value is equally "most common" and the mode
is undefined without binning the data yourself.

**The mode does become useful when you count days rather than measure
depths.** The most common daily rainfall at Silverdale is zero. That is
a real and practically important fact that no mean or median of monthly
totals will ever show you. NOAA gets at it with counts instead: the
station record gives about 102 days a year with 0.10 inches or more, and
about 13.9 days a year with 1.00 inch or more. Add those up across the
months and you learn that roughly 263 days a year fail to reach a tenth
of an inch.

That is the shape of the year in a way the monthly totals hide. It rains
often and usually lightly, and about fourteen times a year it rains
hard. Only 9.5 of those hundred-odd wet days fall in June, July and
August put together.

**The rule to carry away: for anything skewed, ask which average you are
being given.** If a source says "average" without saying which, and the
quantity is rainfall, streamflow, income, or repair cost, it is probably
the mean, and the mean is probably higher than the experience of a
typical year.

### You cannot add medians

One more trap, and it is a good one because the arithmetic proves itself.

Add the twelve monthly means in the table: they come to 56.93 inches,
which is exactly NOAA's published annual precipitation normal for the
station. Means add.

Add the twelve monthly medians: 52.52 inches. That is 4.41 inches below
the annual mean, and it is not the median annual rainfall either.

**The median of a sum is not the sum of the medians.** A "median year"
constructed month by month does not exist, because the wet Januaries and
the wet Augusts are not the same years. If you want a median annual
total, you take the thirty annual totals and find the middle one. You do
not build it out of parts.

## How much does it vary

The topic this document answers promises "the variance," and variance
has its own field in the record.

NOAA publishes a standard deviation alongside each monthly temperature
normal. For station USC00450872, the standard deviation of the monthly
mean temperature, in degrees F, runs from 1.6 in August to 2.6 in
February.

| Month | Mean temp (F) | Std dev (F) |
|---|---|---|
| January | 41.1 | 2.5 |
| February | 42.3 | 2.6 |
| March | 45.7 | 2.3 |
| April | 50.0 | 2.0 |
| May | 56.2 | 2.4 |
| June | 60.7 | 2.1 |
| July | 65.6 | 2.0 |
| August | 66.2 | 1.6 |
| September | 61.5 | 1.7 |
| October | 52.6 | 1.7 |
| November | 45.0 | 2.3 |
| December | 40.3 | 1.9 |

**Winter is less predictable than summer here.** January and February
carry the largest standard deviations, August and September the
smallest. An August in Silverdale is an August. A January can be a mild
wet one or a cold one, and the record says so numerically.

What does a standard deviation of 2.5 F mean in practice? If the
year-to-year variation were normally distributed, roughly two thirds of
Januaries would have a mean temperature within one standard deviation of
41.1 F, so between about 38.6 and 43.6 F. That "if" is doing real work
and I am flagging it rather than hiding it: NOAA publishes the standard
deviation, not a claim that the distribution is normal, and monthly
temperature means are usually close to normal but not guaranteed to be.
Treat the two-thirds figure as a rule of thumb, not a derived result.

**For precipitation, do not use the standard deviation this way at all.**
The distribution is skewed, as the table two sections up demonstrates,
so the symmetric interval that a standard deviation implies would be
wrong in both directions. Use the quartiles instead, which make no
assumption about shape.

The quartiles say something the standard deviation cannot. In July, the
middle half of years falls between 0.15 and 1.27 inches: the wetter end
of the ordinary range is **eight and a half times** the drier end. In
August it is 0.23 to 1.46, a factor of over six. In January it is 6.22
to 12.65, a factor of two.

**So summer rainfall here is not merely small, it is wildly
inconsistent**, and winter rainfall is large but comparatively steady.
A summer water plan that assumes any rain at all is a plan that fails
one year in four. This is the number to hold when reading
[Collecting Rainwater](/library#collecting-rainwater), and it is why storage
volume and not catchment area is the binding constraint in this climate.

## Records, design values, and the 100-year event

Now the extremes, and the single most useful distinction in the whole
document.

Our file gives three numbers at the bottom:

- Record high: 38.9 C (102 F)
- Record low: minus 13.9 C (7 F)
- Winter design temperature: minus 6 C (21.2 F)

**These are different kinds of number and they are not
interchangeable.**

### A record is a historical fact

The record high of 102 F was 28 June 2021, the Pacific Northwest heat
dome. The days on either side read 100 F and 101 F, so it was a real
multi-day event and not a failed sensor. That reading sits about 32 F
above the normal June mean high of 70.2 F.

The record low of 7 F was 21 December 1990, inside a coherent five-day
cold outbreak. Note the year. **December 1990 is outside the 1991-2020
normals window entirely.** The record extremes in our file come from the
full daily record for the station, 1899 to 2026, not from the normals
period. Two different products, two different spans, sitting in the same
file. If you are ever comparing an extreme to a normal, check that you
know which span each one covers.

A record tells you a thing happened once. It does not tell you how
often, and it is the worst possible basis for sizing anything, because
it will be broken. Records are broken by definition: the record is
simply the largest value seen so far, and the sample keeps growing.

### A design value is a probability

The winter design temperature of minus 6 C is a completely different
animal. It is the 99.6 percent annual dry-bulb temperature from NOAA's
Engineering Weather Data for Bremerton National Airport, period of
record 1988-2017.

**"99.6 percent" means the temperature is at or above this value 99.6
percent of the hours in a year.** It is colder than this for the
remaining 0.4 percent, which is about 35 hours a year. That is the
number you size a heating system to. Not the record. The same NOAA table
gives minus 3 C at the 99 percent level, minus 1 C at 97.5 percent, and
a median annual extreme low of minus 9 C.

Look at the gap. The design value is minus 6 C. The record low is minus
13.9 C. **A system sized to the design value will be inadequate on the
coldest night of an exceptional decade, and that is intentional.** Sizing
to the record means buying capacity you use once in thirty years, paying
for it every month in a system that runs inefficiently at part load. The
engineering answer is to size for the design value and have a plan for
the exception: extra blankets, a backup heater, letting the house drift
a few degrees. See
[Insulation and Heat Loss](/library#insulation-and-heat-loss) for what to do
with the number once you have it.

There is a caveat the file states and I will repeat: the design value
comes from an airport station 11 miles away, inland, and about 100 m
higher than Silverdale, so it runs colder. Using it is conservative for
a Silverdale site. A permit application should carry whatever value the
Kitsap County code official requires, not this one.

### The 100-year event, which is not once per century

For rain the equivalent concept is the return period, and it is the most
widely misunderstood number in public life.

The US Geological Survey puts it directly: a 100-year flood is one where
"a flood of that magnitude has a 1 percent chance of happening in any
year." It is an annual probability, not a schedule.

USGS anticipates the misreading and answers it head on. "A 100-year
flood happened last year so it won't happen for another 99 years, right?
Not exactly." And: "100-year floods can happen 2 years in a row."

**The probability does not reset.** Each year is a fresh 1 percent
chance, in the same way that a coin that just came up heads is not
thereby more likely to come up tails. USGS notes that hydrologists
prefer the phrase "a flood having a 100-year recurrence interval," and
increasingly the term annual exceedance probability, or AEP: "An AEP is
always a fraction of one. So a 0.2 AEP flood has a 20% chance of
occurring in any given year, and this corresponds to a 5-year
recurrence-interval flood."

If you are sizing a gutter, a downpipe, a culvert, a swale or an
overflow, the return period is the number you want. A 2-year storm for a
garden swale. A longer return period for anything whose failure damages
a building. You are choosing how much risk to accept per year, and
saying so out loud is the whole value of the concept.

### Where Washington's design rainfall actually comes from

Here the honest answer is worse than you would expect, and you should
know it before you trust any rainfall design figure for this area.

NOAA's modern precipitation frequency product is **NOAA Atlas 14**,
delivered through the Precipitation Frequency Data Server. I queried the
server for the Silverdale coordinates, 47.6448 N, 122.6949 W. It
returned: "Error 3.0: Selected location is not within a project area."

**Washington is not in NOAA Atlas 14.** NOAA's own page for Atlas 2
states that Atlas 2 currently covers "2 states in the western U.S.
(Oregon, Washington)" and remains the current official product for
those two. Every other western state it once covered has moved to Atlas
14: Arizona, Nevada, New Mexico and Utah in 2003, California in 2011,
Colorado in 2013, and Idaho, Montana and Wyoming as recently as 31
August 2024.

NOAA Atlas 2 is the Precipitation-Frequency Atlas of the Western United
States, and Washington is Volume 9. It was published in **1973**. It
gives 6-hour and 24-hour point precipitation at the 2, 5, 10, 25, 50 and
100-year recurrence intervals, with equations for shorter durations.

So if you size a gutter in Kitsap County to a published federal design
rainfall, you are using a figure computed from data that ends before the
1970s, for a quantity that the atmosphere has had fifty years to change.
That is not a reason to ignore it; it is the best official number
available and it is the one a code official will reference. It is a
reason to know its age and to leave margin.

**This is changing.** NOAA Atlas 15 is under development as "the new
authoritative, spatially continuous National Precipitation Frequency
Atlas of the United States." Its headline difference is a shift "from a
stationary assumption ... to a nonstationary assumption (i.e., extreme
precipitation events change over time)," which is to say it stops
assuming the past is a fair sample of the future. NOAA's schedule has
preliminary contiguous-US estimates in September 2026, published
estimates in 2027, and coverage outside the contiguous US in 2028. When
Volume 1 publishes it supersedes Atlas 14, and Washington will finally
leave 1973 behind.

I have not quoted a specific Atlas 2 depth for Silverdale, because I did
not open the Atlas 2 grid or lookup for this point and will not
transcribe a design number I have not personally read. If you need one,
go to the source listed at the end and read it off yourself. A design
value repeated at second hand is exactly the kind of number that should
not be trusted.

## Frost dates are probabilities, not dates

If you take one habit from this document, take this one.

Our file gives three last-spring-frost dates: an "average" of March 29,
an early date of March 5, and a late date of April 24. It gives three
first-autumn-frost dates: average November 14, early October 26, late
December 1. Those are not a range around a true date. **They are three
rungs of a ladder, and NOAA publishes nine.**

Here is the full published ladder for the last spring freeze at the 32 F
threshold, station USC00450872:

| NOAA field | Date | Meaning |
|---|---|---|
| T32FP10 | April 24 | 10 percent of years still have a frost to come |
| T32FP20 | April 15 | 20 percent |
| T32FP30 | April 7 | 30 percent |
| T32FP40 | April 2 | 40 percent |
| T32FP50 | March 29 | 50 percent, the median |
| T32FP60 | March 25 | 60 percent |
| T32FP70 | March 20 | 70 percent |
| T32FP80 | March 13 | 80 percent |
| T32FP90 | March 5 | 90 percent |

Fifty days separate the top rung from the bottom. The first autumn
freeze has its own nine-rung ladder spanning 36 days, from October 26 to
December 1.

### The two conventions, and how to not get them backwards

**This is the single most confusing thing in the whole record, and
getting it backwards is the error that kills crops.**

NOAA names the field by the probability that frost is **still to come**.
April 24 is FP10: a 10 percent probability of a freeze on or after that
date.

Our file, and the guide [The Growing Calendar](/library#the-growing-calendar),
name the same date by the probability that frost is **already finished**.
April 24 is the "90 percent" date there: in 90 percent of years the last
frost has happened by then.

Both are correct. They are complements, and they describe the same day.
If you open NOAA's CSV and see FP10 next to April 24 while our file
calls it `late_90pct`, neither is wrong and you have not found a bug.

The way to never get confused is to stop using percentages as labels and
say the sentence out loud instead. Not "the 90th percentile date" but
**"the date past which only one year in ten still delivers a frost."**
The sentence cannot be read backwards. The label can.

And the practical consequence, which the growing calendar develops
properly: **the average is the 50th percentile, so planting tender crops
to March 29 loses them in half of all years.** Half. That is not a tail
risk, it is a coin flip, and it is what "average last frost date" means
when a seed packet prints it without explanation.

### The threshold matters as much as the probability

Every number above is at the 32 F threshold. NOAA publishes six
thresholds: 16, 20, 24, 28, 32 and 36 F.

The 36 F ladder tells a different story. The one-in-ten late date for
spring at 36 F is **May 13**, nineteen days later than the 32 F date of
April 24.

Why would you use 36 F when water freezes at 32? Because the official
thermometer is not where your plants are. The National Weather Service
explains that on clear calm nights, "super-cooled temperatures can be up
to 10 degrees cooler than 4-5 feet above the surface, where observations
are typically taken," and gives exactly this example: "if conditions are
favorable, air temperatures could be 36 F, but the air in contact with
the surface could be 30 degrees or colder."

**So a station reading of 36 F can mean frost on the ground.** That is
why the 36 F ladder exists and why it is the honest threshold for a
seedling.

The growing season length shows the same thing from the other side. NOAA
publishes it as its own variable, and at the median:

- 32 F threshold: 227 days
- 36 F threshold: 183 days

**Forty-four days of difference, from changing the threshold alone.**
And the 32 F season has its own spread: 257 days at the 10 percent
level, 197 at the 90 percent level, a 60-day range between a long season
and a short one.

One small thing worth knowing, because it looks like an error and is
not: 227 days is NOAA's published median growing season length, but if
you count the days from the median last frost (March 29) to the median
first frost (November 14) you get 230. The median of the season lengths
is not the gap between the median dates, for the same reason the median
of a sum is not the sum of the medians. Both numbers are right. They
answer slightly different questions.

## Growing degree days

Calendar days are a poor measure of how much growing a plant has done,
because a cold April day and a warm one are not equivalent. Growing
degree days replace the calendar with an accumulated temperature count.

NOAA's Climate Prediction Center gives the standard computation: "The
index is computed by subtracting a base temperature of 50 degrees F from
the average of the maximum and minimum temperatures for the day."

There is a truncation rule: "Minimum temperatures less than 50 degrees F
are set to 50, and maximum temperatures greater than 86 degrees F are
set to 86." The reason given is that "these substitutions indicate that
no appreciable growth is detected with temperatures lower than 50 or
greater than 86."

**The base temperature is not universal.** It depends on the crop, and
NOAA publishes the annual accumulation at several bases. For station
USC00450872:

| Base | Annual growing degree days |
|---|---|
| 40 F | 4658.3 |
| 45 F | 3200.0 |
| 50 F | 2058.0 |
| 55 F | 1194.5 |
| 60 F | 567.7 |

Read down that column and you can see the summer being eaten away. A
cool-season crop working from a 40 F base gets 4658 degree days a year
here. A warm-season crop needing a 60 F base gets 568.

**That collapse is the real constraint on growing heat-loving crops in
this climate**, and it is invisible in the frost dates. The frost-free
season is 227 days, which sounds generous. But the season is cool, so a
crop that needs accumulated warmth rather than merely an absence of
frost may still fail in a 227-day window. That is the quantitative
version of the local folk knowledge that tomatoes are hard here and
brassicas are easy.

When you read a seed catalogue that lists "days to maturity," those days
were counted somewhere with a different degree-day accumulation than
yours. Growing degree days are how you translate. Matching them properly
belongs to [The Growing Calendar](/library#the-growing-calendar).

## Your yard is not the station

Everything above describes a thermometer in Bremerton. Here is how far
that can be from your ground.

### The regional picture: rain shadow

Silverdale sits in the lee of the Olympic Mountains, and the size of
that effect is hard to overstate. The National Park Service, describing
Olympic National Park, states that "the west slopes of Mount Olympus
receive about 200 inches (508 cm) of precipitation per year," while
"less than 34 miles to the east, precipitation is less than 20 inches
(50.3 cm) per year." The mechanism is stated in the same place: "The
Olympic Mountains intercept moisture-laden Pacific winds, resulting in a
significant rainshadow effect." Olympic National Park is the wettest
spot in the conterminous United States.

**A factor of ten in rainfall across 34 miles.** Silverdale, at 56.93
inches, sits partway down that gradient: far wetter than Sequim in the
deep shadow, far drier than the west-facing valleys.

The practical consequence is that **regional rainfall figures are
useless here.** "Western Washington gets a lot of rain" is true of a
region containing both the wettest place in the lower 48 and places
drier than parts of the southwest. A number from Seattle, or from the
Hoh, or from a state-level average, does not transfer to Dyes Inlet.
Only a nearby station does, and even then with the reservations below.

### The local picture: water, elevation, and cold air

Three things separate a Silverdale yard from the Bremerton gauge.

**Distance from the water.** Silverdale sits at the head of Dyes Inlet.
Water has an enormous heat capacity and moderates the air above it, so
shoreline sites run warmer on cold nights and cooler on hot afternoons
than sites a few hundred metres inland. The file records this as a
hardiness zone effect and it is measurable: ZIP codes 98383 (Silverdale),
98312 and 98370 are USDA zone 8b, while 98311 and 98337 nearer the
Bremerton waterfront are 9a. **That is a full half-zone across a few
miles of the same town**, and Silverdale sits right on the boundary.

**Elevation.** The station is at 33.5 m. Silverdale's own terrain runs
from sea level to about 178 m, mean about 64 m. A yard near the top of
that range is roughly 145 m above the station.

**Cold air drainage, which reverses what you expect.** The intuitive
correction for elevation is that higher is colder. On clear calm nights,
which is exactly when frost happens, it goes the other way. The National
Weather Service states it plainly: "Cold air will settle in the valleys
since it is heavier than warm air, therefore frost conditions are more
prone in these regions." A low pocket at the bottom of a slope can
freeze while ground 20 m higher does not.

So there is no single correction you can apply. **You cannot adjust the
station record to your yard with arithmetic.** You can only measure.

The same NWS page gives the reason frost is hard to predict from an
official reading at all: clear skies allow "the greatest amount of heat
to exit into the atmosphere," and calm winds "prevent stirring of the
atmosphere, which allows a thin layer of super-cooled temperatures to
develop at the surface." Frost is a surface phenomenon; the thermometer
is at chest height.

### What this means for anyone building or planting

A microclimate can be worth more than a zone. A south-facing wall, a
slope that sheds cold air downhill, a windbreak, proximity to water: any
of these can move a site by more than the difference between the
published zones. That works against you too, in a frost pocket at the
bottom of a hollow.

This is the same reasoning that runs through
[Choosing Where to Build](/library#choosing-where-to-build) and
[Your Soil Specifically](/library#your-soil-specifically), and the same
lesson: the published map is where you start looking, not where you stop.
For a site on the inlet itself,
[Reading the Tide](/library#reading-the-tide) covers the other half of what
the water does to a shoreline property.

## How good is your record

A climate record can be thin, and NOAA tells you when it is. Most people
never look.

Every value in a normals file carries a **completeness flag**. The NCEI
readme defines them:

- **S, Standard**: "meets WMO standards for data availability for 24 or
  more years (missing months are filled with estimates based on
  surrounding stations where available)"
- **R, Representative**: "meets WMO standards for data availability for
  10 or more years (missing months are filled with estimates based on
  surrounding stations)"
- **P, Provisional**: 10 or more years, but missing months cannot be
  filled for lack of surrounding stations
- **E, Estimated**: 2 or more years, with normals estimated
  statistically from nearby stations

**Every frost-date and growing-degree-day value for Bremerton carries
the R flag.** Ten years, not twenty-four. The monthly temperature and
precipitation normals mostly carry S, with two exceptions: November
precipitation and December average temperature are R.

Each value also carries a count of the years actually used. For the
monthly temperature normals at this station that count runs from 23 to
27, never 30. For precipitation, 23 to 26.

This is worth sitting with. **The frost dates that every gardener in
Kitsap County quotes come from a station that cleared a 10-year bar, not
a 24-year bar, and monthly normals built from around 25 years of real
observation rather than 30.** The estimates are sound, they are the best
available, and they are not a thirty-year unbroken record. Our file says
so too: 20 of the 30 years have a near-complete daily record, and 1991,
1997, 2002 and 2009 have substantial gaps NOAA filled from surrounding
stations.

None of this makes the numbers wrong. It sets how hard you should lean
on the third significant figure. **When your record is flagged R, treat
April 24 as "late April," not as a date.**

## A lesson in rounding, hiding in our own file

Here is something you can verify in two minutes, and it teaches a real
habit.

Our file says annual precipitation is 1446 mm. Add the twelve monthly
values in the same file: 236 + 148 + 162 + 98 + 60 + 41 + 21 + 28 + 46 +
131 + 225 + 252. They come to **1448 mm**.

Two millimetres missing. Is the file wrong?

No, and tracking down why is instructive. NOAA publishes this station's
normals in inches. The twelve monthly values are 9.28, 5.83, 6.37, 3.86,
2.36, 1.61, 0.83, 1.09, 1.80, 5.14, 8.84, 9.92, and they sum to exactly
56.93 inches, which is exactly NOAA's published annual normal. The
source data is perfectly consistent.

The 2 mm appears during conversion. Each monthly value in inches becomes
a fractional number of millimetres, and each was rounded to a whole
millimetre. Nine of the twelve rounded up and only three rounded down.
November alone gained 0.46 mm (224.54 rounds to 225), October 0.44 mm,
August 0.31 mm, while February, April and July together gave back only
0.20 mm. **Net it out and you get 1.98 mm of rain that never fell.**

The annual figure, 1446, is the correctly converted annual normal:
56.93 inches is 1446.02 mm. The 1448 is an artifact of adding rounded
parts.

**The habit: the sum of rounded numbers is not the rounded sum.** If a
total has to match its parts, convert once at the end, or keep a
decimal. If you are quoting both, say which one you did.

[Units and Converting Them](/library#units-and-converting-them) develops this
properly, including the principle underneath it: a conversion cannot add
precision the original measurement did not have.

**There is a genuine defect here**, and it is in the prose rather than
the data. The explanatory note in `climate.json` says the annual figure
"matches the sum of the twelve monthly normals exactly," which is true
in inches and false in the file's own millimetre column, and a later
sentence in the same note refers to "the 1,448 mm the monthly table sums
to." Both sentences are individually defensible and together they are
confusing. The values are correct; the note needs a line saying which
unit it is talking about. It is reported rather than fixed here, so the
correction gets its own reasoning.

## Where to look up your own numbers

Our file is Silverdale. If you live somewhere else, or want to check us,
here is the actual path.

**Start at NOAA NCEI, US Climate Normals.**
`https://www.ncei.noaa.gov/products/land-based-station/us-climate-normals`
is the product page. It describes what normals are, what periods are
available, and links the access tools.

**Find your station, not your town.** Normals are computed per station.
You need a station identifier such as USC00450872. Search the access
tools by place and take the nearest station that actually carries the
variable you want. As Silverdale demonstrates, the nearest station is
often precipitation-only, and the nearest one with temperature and frost
data may be miles further.

**Pull the raw record if you want everything.** Two URLs, with your
station ID substituted:

- Monthly: `https://www.ncei.noaa.gov/data/normals-monthly/1991-2020/access/<ID>.csv`
- Annual and seasonal: `https://www.ncei.noaa.gov/data/normals-annualseasonal/1991-2020/access/<ID>.csv`

The monthly file for Bremerton has 413 columns. That is not a
misprint, and it is the point: the headline table you usually see is a
dozen numbers out of hundreds. The percentiles, the standard deviations,
the day counts and the completeness flags are all in there.

**Read the variable documentation.** The readme at
`https://www.ncei.noaa.gov/data/normals-annualseasonal/1991-2020/doc/Readme_By-Variable_By-Station_Normals_Files.txt`
decodes the naming scheme. It is how you learn that the agricultural
variables encode "temperature threshold and probability in the form
TnnFPmm where nn is the temperature and mm is the probability," which is
what lets you read T32FP10 without guessing.

**Consider the 15-year normals.** NCEI also publishes a full 2006-2020
set, computed by the same methods, "optimized for use cases that require
more recent climate information, such as predicting energy system loads
and other economic decisions." If you suspect the last fifteen years
differ from the last thirty, you can check rather than speculate.

**For design rainfall, go to the Precipitation Frequency Data Server**
at `https://hdsc.nws.noaa.gov/pfds/`. It will tell you honestly if your
location is outside a project area, as it did for Silverdale. For
Washington and Oregon, the current product is NOAA Atlas 2 at
`https://www.weather.gov/owp/hdsc_noaa_atlas2`.

**For local expertise, find your NWS Weather Forecast Office.** Western
Washington including Kitsap County is served by the NWS Forecast Office
Seattle/Tacoma at `https://www.weather.gov/sew/`, which carries local
climate pages and regional observations. The forecast office is also who
you ask when a record looks wrong.

**For the hazard side of the same question**, the extremes that are
events rather than statistics, see
[The Hazards Where You Live](/library#the-hazards-where-you-live).

## What to measure yourself, and why it is worth it

Go back to the can on the lawn.

A station record five miles away tells you the shape of the year. It
does not tell you that the north side of your house never dries out,
that the bottom of your garden frosts two weeks later in spring than the
top, or that the wind comes reliably from the southwest in the storms
that matter. Those are facts about your ground and nobody has collected
them.

**A minimum useful record is three things, once a day.** A minimum and
maximum thermometer read each morning. A rain gauge read at the same
time. A note of anything notable: first frost, last frost, first
blossom, a storm, snow that stuck.

Do that for one year and you have something no download gives you. Do it
for five and you can start to say how your site differs from the
station, which is the only correction that will ever be right. **Put the
thermometer where your question is.** If the question is about frost on
seedlings, it goes at seedling height in the bed, not on the porch at
chest height, for the reason the NWS gave above.

Two disciplines make the record worth keeping. Read at the same time
every day, because a maximum thermometer read at dusk and one read at
dawn are measuring different days. And **write down the zeros.** A rain
gauge that is only recorded when it has water in it produces a record
where it never stops raining, which is the same skew problem as the mean
and the median, manufactured by hand.

Silverdale's phenology record in this project, at
`data/locales/silverdale_wa/phenology.json`, is a worked example of what
observations look like once they are graded for reliability: events are
marked `measured`, `published_range`, `local_consensus` or `estimated`,
and those are not the same claim. Your own notebook is the `measured`
column for your own ground.

## The four questions, answered

For Silverdale, at the end of all that:

**What is normal?** A mild wet winter and a dry warm summer. Annual mean
52.3 F. Monthly means from 4.6 C (40.3 F) in December to 19.0 C (66.2 F)
in August. Annual precipitation 56.93 inches, about 80 percent of it
between October and March. About 102 days a year with measurable rain.
The normals cover 1991-2020 and will be recomputed for 2001-2030.

**How much does it vary?** Temperature varies little: a standard
deviation of 1.6 to 2.6 F on the monthly means, largest in winter.
Rainfall varies enormously, and most in summer: the middle half of Julys
spans 0.15 to 1.27 inches, a factor of eight and a half. The frost-free
season ranges from 197 to 257 days.

**What is the extreme?** Record high 102 F (June 2021), record low 7 F
(December 1990). But the number to build to is the design value: minus 6
C for heating, the temperature it is colder than for about 35 hours a
year. For rainfall, the official design figures come from a 1973
publication, and NOAA Atlas 15 will replace them from 2026.

**Where do I look it up?** NOAA NCEI for normals, by station ID, raw CSV
if you want the percentiles and flags. The NWS Seattle/Tacoma office for
local context. And a rain gauge in your own yard for the thing none of
them can tell you.

**The single most useful habit in all of this:** when you are handed a
climate number, ask which of the four questions it answers. Most
arguments about climate data are two people answering different
questions with numbers that were never meant to be compared.

## Sources

Every URL below was opened while writing this document. Where something
could not be sourced, it says so.

**NOAA NCEI, US Climate Normals product page.**
`https://www.ncei.noaa.gov/products/land-based-station/us-climate-normals`
Gave the definition of a normal as a 30-year average over a uniform
period; confirmation that the current set is 1991-2020, first released
May 2021; the decadal recomputation schedule and the WMO requirement;
and the existence and rationale of the supplemental 2006-2020 15-year
normals. This page does not state that normals are not predictions; that
framing here is drawn from what the product is, not quoted from NOAA.

**NOAA NCEI, 1991-2020 Monthly Normals, station USC00450872 (Bremerton,
WA).**
`https://www.ncei.noaa.gov/data/normals-monthly/1991-2020/access/USC00450872.csv`
Downloaded and parsed directly. Source of every monthly figure used
here: means, the 25th/50th/75th precipitation percentiles, the
temperature standard deviations, the day counts at the 0.10 and 1.00
inch thresholds, the completeness flags and the per-variable year
counts. 413 columns, 12 rows.

**NOAA NCEI, 1991-2020 Annual/Seasonal Normals, station USC00450872.**
`https://www.ncei.noaa.gov/data/normals-annualseasonal/1991-2020/access/USC00450872.csv`
Downloaded and parsed directly. Source of the full nine-rung frost
ladders at the 28, 32 and 36 F thresholds, the growing season lengths,
the annual precipitation normal of 56.93 inches, the annual mean
temperature of 52.3 F, and the growing degree day accumulations at bases
40 through 72.

**NOAA NCEI, Normals variable documentation readme.**
`https://www.ncei.noaa.gov/data/normals-annualseasonal/1991-2020/doc/Readme_By-Variable_By-Station_Normals_Files.txt`
Gave the TnnFPmm naming convention and the worked FP10 example; the
definitions of PRBLST, PRBFST and PRBGSL; the full completeness flag
definitions (S, R, P, E) with their year thresholds; and the note that
quartiles in the by-variable files appear as percentiles in the
by-station files.

**USGS Water Science School, "The 100-Year Flood."**
`https://www.usgs.gov/special-topics/water-science-school/science/100-year-flood`
Gave the 1 percent annual chance definition, the explicit correction
that a 100-year flood does not mean once per century and can happen two
years running, the preference for "recurrence interval," and the
annual exceedance probability definition with the 0.2 AEP example.

**NOAA NWS Office of Water Prediction, NOAA Atlas 2.**
`https://www.weather.gov/owp/hdsc_noaa_atlas2`
Gave the fact that Atlas 2 remains the current official precipitation
frequency product for Oregon and Washington only, the 6-hour and 24-hour
durations, and the dates on which each other western state was
superseded by Atlas 14, including Idaho, Montana and Wyoming on 31
August 2024.

**NOAA NWS HDSC, Washington precipitation frequency page.**
`https://hdsc.nws.noaa.gov/pfds/other/wa_pfds.html`
Confirmed Washington is NOAA Atlas 2 Volume 9, published 1973, with
return periods of 2, 5, 10, 25, 50 and 100 years.

**NOAA NWS HDSC, Precipitation Frequency Data Server.**
`https://hdsc.nws.noaa.gov/pfds/` and the point query
`https://hdsc.nws.noaa.gov/cgi-bin/hdsc/new/fe_text_mean.csv?lat=47.6448&lon=-122.6949&data=depth&units=english&series=pds`
The point query for the Silverdale coordinates returned "Error 3.0:
Selected location is not within a project area," which is the primary
evidence quoted above that Silverdale is outside NOAA Atlas 14. The PFDS
landing page itself is script-driven and returned no coverage statement.

**NOAA, NOAA Atlas 15 information page.**
`https://water.noaa.gov/about/atlas15`
Gave the description of Atlas 15 as the new authoritative spatially
continuous atlas, the shift from stationary to nonstationary
assumptions, the Volume 1 and Volume 2 split, and the 2026/2027/2028
schedule.

**NOAA Climate Prediction Center, Growing Degree Day explanation.**
`https://www.cpc.ncep.noaa.gov/products/analysis_monitoring/cdus/degree_days/gdd.shtml`
Gave the base-50 computation, the 50 and 86 degree truncation rules, and
the stated reason for them. This page discusses corn specifically and
does not list base temperatures by crop; the multiple bases quoted here
come from the NCEI annual normals file, not from this page.

**NOAA NWS La Crosse, "What Causes Frost?"**
`https://www.weather.gov/arx/why_frost`
Gave radiational cooling under clear skies, calm winds preventing
mixing, cold air settling in valleys, the 4 to 5 foot standard
observation height, and the worked example of 36 F at observation height
with 30 F or colder at the surface.

**National Park Service, Inventory and Monitoring at Olympic National
Park.**
`https://www.nps.gov/im/nccn/olym.htm`
Gave the rain shadow figures: about 200 inches (508 cm) on the west
slopes of Mount Olympus against less than 20 inches (50.3 cm) under 34
miles east, the statement that Olympic is the wettest spot in the
conterminous United States, and the orographic mechanism.

**USGS, Ecology of Olympic National Park.**
`https://www.usgs.gov/geology-and-ecology-of-national-parks/ecology-olympic-national-park`
Confirmed the temperate rain forests receive 140 to 167 inches of rain a
year. It did not carry the east-west comparison, which is why that is
cited to NPS above.

**NOAA NWS Forecast Office Seattle/Tacoma.**
`https://www.weather.gov/sew/` and
`https://www.weather.gov/sew/Cliplot`
Confirmed the office identity and address and that it carries local
climate and observation sections. Both pages are navigation shells whose
content loads elsewhere, so neither yielded a Kitsap County climate
product directly; they are cited only as the correct office to consult.

**Project data files**, read directly, not fetched:
`data/locales/silverdale_wa/climate.json` (the station note, the
extremes, the winter design temperature and its NOAA Engineering Weather
Data provenance, the hardiness zone detail and the ZIP-code boundary,
the Koppen computation, the data completeness note) and
`data/locales/silverdale_wa/locale.json` (the centre coordinates
47.6445 N 122.6949 W and the elevation range 0 to 178 m, mean 64 m).

### What could not be sourced

**A specific Atlas 2 design rainfall depth for Silverdale.** I
established which publication governs and what it contains, but did not
open the Atlas 2 grid or lookup for this point, so no depth is quoted.
A design value repeated at second hand is worse than none.

**The NOAA explainer page on climate normals**
(`https://www.noaa.gov/explainers/climate-normals-explained`) returned
HTTP 403 and is not cited. Nothing here rests on it.

**The claim that a 130-foot elevation difference produces a large
nighttime temperature difference** appeared in a search summary of an
NWS training page I did not open, so it is not used. The qualitative
statement about cold air settling in valleys is cited to the NWS page I
did open.

**Whether the roughly 25-year counts behind these normals materially
shift the frost dates** is not something I could establish. The record
carries R flags and year counts in the low-to-mid twenties; how much
that moves any individual date is not published, and I have not
attempted to derive it.
