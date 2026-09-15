# The Sky as an Instrument

Go outside tonight, find one star, and hold your fist up next to it.
Count how many fists it sits above the horizon. That number, times ten,
is your latitude.

That is not a memory aid or a party trick. It is the real measurement,
and it is good to within about a degree. Sailors crossed oceans on it
for centuries before anyone could do better, and nothing has happened
since to make it stop working.

This guide is about the small number of genuinely useful things the sky
will tell you, and it is equally careful about the things it will not.
The sky gives you latitude, direction and season very well. It gives you
time badly. It gives you longitude not at all unless you are carrying an
accurate clock, and that is a fact worth saying out loud early, because
a great deal of folklore implies otherwise.

Everything numbered here was computed by the US Naval Observatory, NOAA
or NIST for this specific place, and every one of those numbers is
public. Where something is a working rule rather than a published
figure, the text says so.

A note on how things are written. This file stays plain ASCII, so angles
are written "47.6 degrees" rather than with a degree symbol, and times
are on a 24 hour clock. Local numbers are for Silverdale, Washington, at
47.6445 degrees north and 122.6949 degrees west, which is the place the
simulation is built on.
[Where You Are: Silverdale, Washington](../locale/silverdale_wa.md) has
the rest of the setting. The method transfers anywhere on Earth. The
numbers do not.

## Tonight, with nothing in your hands

Wait for full dark, get away from a streetlight if you can, and face
north. Find the Big Dipper. Find Polaris using the two stars at the
outer lip of the Dipper's cup, which is the next section. Then estimate
how high above the horizon Polaris sits.

At Silverdale it will be a little over halfway from the horizon to
straight overhead. Not quite five fists at arm's length. If you measure
it and get something near 48 degrees, you have just determined your
latitude from first principles, standing in your yard, with no
instrument at all.

Then do the other half. Polaris is within one degree of true north. Not
magnetic north, which your phone and your compass both give you and
which is wrong here by about fifteen degrees. True north, the one your
map is drawn to. Look at Polaris, drop your eyes to the horizon below
it, and pick a landmark. That landmark is north of you, and it will
still be north of you next year.

Two useful facts, one night, no equipment. Everything after this is
detail, accuracy and honesty about the limits.

## Finding Polaris

Polaris is not bright. This surprises people who have been told it is
the North Star, as though that meant the brightest. It is about as
bright as the stars of the Big Dipper itself, which is to say clearly
visible from a dark yard and often visible from a lit street, but not
remarkable.

NASA gives the method in one sentence: find the Big Dipper, and the two
stars on the end of the Dipper's cup point the way to Polaris, which is
the tip of the handle of the Little Dipper.

Those two stars are called Dubhe and Merak, and the gap between them is
your ruler. NASA puts that gap at about 5.5 degrees, which it describes
as roughly three middle fingers held at arm's length. The star catalogue
this project ships, `data/stars.csv`, puts the two stars 5.374 degrees
apart. Those agree, which is a small thing but a satisfying one: the
figure in a NASA article and the figure computed from the catalogue the
game draws its sky from are the same figure.

Now extend the line. Start at Merak, the star at the bottom of the cup,
run through Dubhe at the lip, and keep going in that direction for about
five more gaps. Polaris is 28.7 degrees beyond Dubhe, which is 5.3
gap-widths, so the folk rule of "about five times" is right.

Nothing else bright is anywhere near there. If you have extended the
line and landed on a lonely middling star with empty sky around it, that
is Polaris.

Two checks, if you want them. Polaris should be at the end of the handle
of a much fainter dipper shape, the Little Dipper, whose two brightest
remaining stars are Kochab and Pherkad. Kochab sits 16.6 degrees from
Polaris. And Polaris should not move. Come back an hour later and every
other star will have swung; Polaris will be where you left it.

### When the Dipper is low

At Silverdale the Big Dipper never sets. A star stays above the horizon
all night from here if its declination is greater than 42.36 degrees,
which is 90 minus the latitude, and the lowest star of the Dipper is
Alkaid at 49.31 degrees. So the whole thing is technically up all night,
every night of the year.

Technically. On autumn evenings the Dipper swings down into the north
and Alkaid drops to about 7 degrees above the true horizon, which means
it is behind your neighbour's trees, or behind the ridge, or lost in the
glow off Bremerton. Being above the mathematical horizon and being
visible are different things.

What saves you is that the pointers survive. Dubhe, the star you
actually need, never gets lower than about 19 degrees from here. So even
on the worst night for the Dipper, the two pointer stars are usually
still clear of the treeline even when the handle is gone.

If they are not, use Cassiopeia instead. It is the obvious W or M shape
of five stars on the opposite side of Polaris, at almost the same
distance: Schedar, its brightest, is 32.8 degrees from Polaris against
Merak's 34.1. The two are a seesaw. When the Dipper is scraping the
northern horizon in autumn, Cassiopeia is nearly overhead, riding within
about 9 degrees of the zenith. When the Dipper is high in spring,
Cassiopeia is the one in the trees.

Polaris sits roughly on the line between them. That is the whole trick,
and between the two of them you have a way to the pole on any clear
night of the year.

One thing you do not have, from here: the Southern Cross. Crux is in the
game's constellation file, and every guide to navigating by the stars
mentions it, and from 47.6 degrees north it never rises. Its brightest
star, Acrux, sits at declination -63.1, and anything below -42.36 stays
under the horizon from this latitude permanently. Do not spend a night
looking for it.

## Why the pole's height is your latitude

This is the one genuinely magical fact in the subject, and it is worth
understanding rather than memorising, because then you can rebuild it
when you need it.

Stand at the North Pole. The Earth's axis runs straight up through you.
The point in the sky that the axis points at, the north celestial pole,
is therefore directly overhead, at 90 degrees of altitude. Your latitude
is 90 degrees north. The two numbers match.

Now stand on the equator. The axis runs horizontally past you, north to
south. The celestial pole is on your horizon, at 0 degrees of altitude.
Your latitude is 0. The two numbers match again.

They match everywhere in between, and for the same reason: as you walk
south from the pole, the axis tilts away from vertical by exactly the
angle you have walked around the Earth's curve. Move one degree of
latitude south and the celestial pole drops one degree toward your
horizon. There is no fudge factor and no scaling constant. The altitude
of the north celestial pole above your horizon is your latitude, exactly.

Polaris is useful only because it happens to sit almost on that point.

Here is the proof, using this specific place. The table below is the
altitude of Polaris at Silverdale at five moments spread across 2026, as
computed by the US Naval Observatory's own celestial navigation service.
The true latitude is 47.6445 degrees.

| Date and time (UT) | Polaris altitude | Error against latitude |
|---|---|---|
| 2026-06-21 12:13 | 47.793 | +0.149 |
| 2026-06-21 20:13 | 48.092 | +0.448 |
| 2026-09-22 13:58 | 48.114 | +0.470 |
| 2026-12-21 15:56 | 47.065 | -0.580 |
| 2026-12-21 20:09 | 47.188 | -0.456 |

Every one of those is within six tenths of a degree of the right answer,
at any hour, in any season, with no correction applied and no equipment
beyond something to measure an angle with.

## How wrong Polaris is, and when that matters

The errors in that table are not noise and they are not random. Polaris
is not at the celestial pole. It is near it, and it circles it once a
day, so its altitude swings above and below your latitude on a
twenty four hour cycle.

The size of that circle is the whole story. In 2026 the Naval
Observatory puts Polaris at declination 89.381 degrees, so it sits
0.619 degrees from the pole. That is 37 minutes of arc, or about the
width of your little fingernail at arm's length.

So: **the altitude of Polaris is your latitude, plus or minus 0.62
degrees.** That is the honest statement. On the ground it is a worst
case of about 69 kilometres north or south, using the Earth's polar
circumference of 40,009 kilometres that NOAA quotes, which works out to
111 kilometres per degree of latitude.

Sixty nine kilometres sounds terrible until you ask what you were going
to do with the number. If you are trying to work out which climate zone
you are in, what day length to expect, or roughly where on a continent
you have ended up, an error of 0.6 degrees is nothing. If you are trying
to make a landfall, it is a disaster. Know which question you are asking.

Three ways to do better, in increasing order of effort.

**Average it.** The error is a circle, so it cancels. Measure Polaris,
wait about twelve hours, measure it again, and take the mean of the two.
Most of the offset disappears. This is a rule of thumb rather than a
published procedure, but it follows directly from the geometry. The table
above does not happen to contain a twelve-hour pair to demonstrate it,
and it is worth saying so rather than pointing at the nearest thing: the
two December readings are only four hours and thirteen minutes apart, and
their mean of 47.13 is no better than the better of the two alone. The
December and September readings land on opposite sides of the circle by
coincidence of phase rather than by this procedure, and average to 47.59,
which is within six hundredths of a degree of the truth.

**Watch for the crossings.** Twice a day Polaris passes directly above
or directly below the pole, and at those two moments the error is at its
maximum. Halfway between them, when Polaris is at its furthest to one
side, its altitude equals your latitude exactly. You can see this in the
table too: the 2026-06-21 12:13 reading, where Polaris was 0.91 degrees
east of north, is the one closest to the true latitude.

**Use the correction table.** The Nautical Almanac publishes a set of
Polaris corrections for exactly this, indexed by the local hour angle of
Aries. That is the professional answer, and the next section explains
why this guide cannot simply reprint it.

### Precession: Polaris has not always been the pole star

The Earth's axis wobbles like a slowing top. NASA puts the period at
about 26,000 years, and the consequence is that the celestial pole
crawls in a slow circle among the stars, so the pole star changes.

NASA's own example: about 14,000 years ago the pole pointed at Vega,
and it will point at Vega again in about 12,000 years. The constellation
file this project ships carries the same story in its note on Lyra, with
one figure to distrust, because it gives the past epoch as about 12,000
years ago where NASA gives 14,000, and its
note on Draco records that Thuban held the job when the pyramids were
built.

You do not need to care about a 26,000 year cycle. You should care that
it is fast enough to show up in a human lifetime, and there is a clean
demonstration sitting in this repository.

The game's star catalogue lists positions for epoch J2000, the standard
reference frame for the year 2000. It puts Polaris at declination
89.2641, which is 0.736 degrees from the pole, or 44 minutes of arc.
The Naval Observatory, computing where Polaris actually is in 2026, puts
it at 89.381, which is 0.619 degrees, or 37 minutes.

Polaris has moved seven minutes of arc closer to the pole in
twenty six years. It is still closing, and it will keep closing for
roughly another century before it starts back out again. So if you take
a Polaris measurement off the in-game sky, you are working with the
year 2000 pole star, not tonight's, and it sits about a tenth of a
degree further out than the real one. That is smaller than the
measurement error of a fist, but it is real, and it is the kind of thing
worth knowing you have.

## Measuring an angle with your hand

You cannot use any of this without a way to measure an angle, and the
one you always have with you is your hand at arm's length.

The NASA Night Sky Network publishes the calibration. A little finger
covers about 1 degree. Three middle fingers together cover about
5.5 degrees. A hand spread from thumb to little finger covers about
25 degrees. And a full moon, usefully, is about half a degree across,
which is half the width of your little finger.

The usual fourth step in that ladder is a closed fist at about
10 degrees. That one is a rule of thumb rather than a figure the Night
Sky Network prints, and it is the least reliable of the set, because
fist size and arm length vary between people more than finger width
does.

Which is why the right move is to calibrate your own hand rather than
trust anybody's table, including this one. You have two free rulers
overhead:

- **The pointer gap.** Dubhe to Merak is 5.374 degrees, from the
  catalogue this project ships. Hold your fingers up against it.
- **Merak to Polaris.** 34.07 degrees, from the same file. Count how
  many of your own fists or spans fit in that gap and divide.

Do it once, write the numbers on the inside of a notebook, and your hand
is a calibrated instrument for the rest of your life. Note that it is
calibrated for *your* arm, which is the point; a borrowed table is
calibrated for somebody else's.

The reason arm's length works at all is that people with long arms tend
to have big hands. The ratio is roughly constant even though neither
measurement is. That is a general fact about human proportion rather
than a published constant, so treat it as the reason the trick works and
not as a guarantee that it works for you specifically. Check yours.

## Direction by night

Polaris is true north, near enough. How near is a number you can state
exactly.

Polaris circles the pole at 0.619 degrees. At Silverdale's latitude that
sideways swing works out to a maximum of 0.918 degrees of azimuth, and
the Naval Observatory's computed values bear it out: across the five
sample times above, Polaris ranged from 0.908 degrees east of true north
to 0.658 degrees west of it.

**So Polaris is within one degree of true north, always, from this
latitude.** For every practical purpose on land, that is true north. It
is roughly fifteen times better than the compass in your pocket, which
is off by about fifteen degrees here for an entirely different reason
covered below.

The procedure is worth doing deliberately rather than by eye. Stand
somewhere you can return to. Find Polaris. Without moving your feet,
lower your gaze straight down to the horizon and pick the most permanent
thing you can see there: a chimney, a particular tree, the corner of a
roof. Go back in daylight and confirm you can still identify it. That
landmark is now a true north reference you can use at any hour, in any
weather, for as long as the tree stands.

If you also want east and west: face Polaris, and east is your right
hand, west your left, south behind you. That is exact, not approximate,
once you are facing north.

## Direction by day

The sun is a worse compass than Polaris, because it moves. But it is
usable in two quite different ways, one of which is precise and one of
which is not, and the difference between them is worth being clear
about.

### The shadow at local noon

At the moment the sun is highest, it is due south. Not roughly south:
due south, from anywhere north of the Tropic of Cancer, which includes
every part of the United States except Hawaii and the southern tip of
Florida.

The Naval Observatory's computed azimuth for the sun at its highest
point at Silverdale is 180.18 degrees on the June solstice and 179.997
degrees on the December solstice. Due south is 180. That is the whole
fact.

So a vertical stick's shadow, at the instant it is shortest, points due
**true** north. Not magnetic north. The real one, the one Polaris gives
you at night, with no declination correction needed.

To use it: push a straight stick into level ground, as close to vertical
as you can get it (check it against a plumb line made of a string and a
rock, because leaning the stick tilts the answer). Mark the shadow tip
every few minutes through the middle of the day. The shortest of those
marks gives you the north line.

That method is slow and it costs you an hour of standing around, and it
is the most accurate direction-finding you can do in daylight without
instruments. It is worth doing once, properly, to establish a permanent
north line at a place you will keep coming back to.

Note the trap hidden in it: **the shortest shadow does not happen at
noon on the clock.** At Silverdale it happens anywhere from 11:54 to
12:25 on standard time, and around 13:13 in June when the clocks are on
daylight saving. The section on time explains why. For now: watch for
the shortest shadow, do not look at your watch and assume.

### The shadow-tip line, and why it is only approximate

The faster version, which turns up in every survival manual, is to mark
the shadow tip, wait fifteen minutes to an hour, mark it again, and draw
a line between the two marks. That line runs roughly east and west, with
the first mark on the west end.

It works, roughly, and it is worth knowing. But it is **a rule of
thumb**, and it is worth knowing why, because the error is not random.

Over the course of a day the shadow tip does not trace a straight line.
It traces a curve, and the curve is a straight line only on the two
equinoxes, when the sun's declination is zero. On every other day of the
year the curve bends, and a chord drawn across a bent curve is not
parallel to the axis you wanted.

The bend is worst near the solstices and worst at high latitudes, both
of which describe this place in December. Silverdale in midwinter is
close to the worst case for this method. Near the equinoxes in March and
September it is genuinely good.

Two practical consequences. Take your two marks reasonably close to
midday, where the curve is flattest and most symmetric about the north
line. And if the answer matters, spend the extra hour and find the
shortest shadow instead.

### What the sun actually does at 47.6 degrees north

"The sun rises in the east and sets in the west" is true twice a year
and wrong the rest of the time, and at this latitude it is wrong by a
lot. Here is what actually happens, from the Naval Observatory's
computed azimuths for this exact spot:

| Date | Sunrise azimuth | Sunset azimuth | Noon altitude | Day length |
|---|---|---|---|---|
| June solstice, 21 Jun 2026 | 52.7 (northeast) | 307.5 (northwest) | 65.8 | 15 h 59 m |
| September equinox, 22 Sep 2026 | 88.9 (east) | about 271 (west) | about 42 | 12 h 10 m |
| December solstice, 21 Dec 2026 | 125.0 (southeast) | about 235 (southwest) | 18.9 | 8 h 26 m |

The sunset azimuths at the equinox and the December solstice, and the
equinox noon altitude, are arithmetic done here by mirroring the
measured sunrise values about the north-south line; the rest are
computed values.

Read the first column again. **In June the sun rises 37 degrees north of
east and sets 37 degrees north of west.** In December it rises 35
degrees south of east. Somebody who steps out at 05:00 on a June morning
at Silverdale, sees the sun coming up and takes that for east, has made
a 37 degree error, which is more than twice the error of an uncorrected
compass.

The other thing that table shows is how much more of the horizon the
summer sun covers. In June it sweeps 254.8 degrees of azimuth from
rising to setting, better than two thirds of the way round the compass.
In December it sweeps 110.0 degrees, a shallow scrape across the
southern sky. Same sun, same place, and a completely different shape of
day. This is also why a solar panel or a window that works in December
faces a quite different direction from one optimised for June.

The noon altitude column is the third useful thing. The sun is never
overhead here, ever. At its highest, on the June solstice, it still
stops 24 degrees short of the zenith, and on the December solstice it
crawls to 18.9 degrees, which is under two fists above the horizon. That
is why winter shadows here are so long, why low winter sun through a
south window heats a room so effectively, and why anything north of a
tall object is in shade all winter.

### The star that rises due east

There is one more direction trick, and it is the most precise thing in
this guide after Polaris.

A star whose declination is zero sits on the celestial equator, and a
star on the celestial equator rises due east and sets due west from
everywhere on Earth. Not approximately, and not only at certain
latitudes. Everywhere except the two poles, where such a star neither
rises nor sets but circles the horizon.

Orion's Belt has one. Mintaka, the westernmost of the three belt stars,
sits at declination -0.299 in the game's own catalogue, which is a third
of a degree off the equator. Worked through the geometry for Silverdale,
that puts its rising point at azimuth 90.44, which is within half a
degree of due east.

So on any clear winter night: watch where Mintaka comes up over the
horizon, and that is east, to half a degree. Watch where it goes down,
and that is west. Orion is the most recognisable constellation in the
northern winter sky and it comes with a free east-west line built in.

## True north is not magnetic north

Everything above gives you **true** north, the direction of the Earth's
axis and the direction your map is drawn to. A compass does not give you
that. A compass points at the magnetic field, which is a different thing
in a different place, and the angle between them is called magnetic
declination.

**At Silverdale, on 15 September 2026, the declination is 14.95 degrees
east.** That is NOAA's World Magnetic Model WMM-2025, queried for
47.6445 north, 122.6949 west. The same query reports an uncertainty of
0.38 degrees and an annual change of -0.124 degrees per year.

Fifteen degrees is not a subtlety. Walk one kilometre on a bearing you
believe is true north but is actually magnetic north, and you finish
about 258 metres to the side of where you meant to be. Walk five kilometres
and you are 1.3 kilometres out. On the Kitsap Peninsula that is the
difference between hitting a road and hitting a different drainage.

"East" declination means magnetic north is 15 degrees east of true
north. So to convert a magnetic bearing off your compass into a true
bearing for your map, you add 14.95. To go the other way, from a true
bearing on the map to the compass bearing you must walk, subtract it.
The USGS describes both ways of handling it: either do that arithmetic
in your head on every reading, or set the declination once on a compass
that has an adjustable housing and then read true bearings directly. The
second is much less error prone and is worth the price of the compass.

Two things about declination that trip people up.

**It changes with place.** Fifteen degrees east is a Puget Sound number.
The line of zero declination runs through the middle of the United
States, and on the east coast declination is westerly. There is no
national value. Look up the one for where you are standing.

**It changes with time.** NOAA replaces the World Magnetic Model every
five years, precisely because the field moves. The current model was
released on 17 December 2024 and expires on 31 December 2029. At
Silverdale's current rate of -0.124 degrees per year, declination here
will be about 13.2 degrees east by 2040. A declination printed on the
margin of an old topographic map is a historical record, not a current
value.

To look up any other place, the same NOAA calculator takes any latitude
and longitude:
https://www.ngdc.noaa.gov/geomag/calculators/magcalc.shtml

And the reason this section sits in a guide about the sky: **the sky is
how you check the compass.** Polaris and the noon shadow both give you
true north directly, with no model, no correction and no batteries. If
your compass and the noon shadow disagree by 15 degrees at Silverdale,
the compass is right and working. If they disagree by 40, something near
you is magnetic, and you should walk away from it and try again.

## Time from the sky, and what it cannot give you

The sky keeps excellent time. Clocks do not keep the sky's time, on
purpose, for three separate reasons that stack.

### Solar noon is not clock noon, three separate reasons

**One: your longitude.** A time zone is one time for a wide strip of
land, so almost nobody in it is on the meridian the zone is named for.
Pacific Standard Time is set on 120 degrees west. Silverdale is at
122.6949 west, which is 2.69 degrees further west. The Naval Observatory
gives the conversion: mean solar time runs 4 minutes later for each
degree of longitude west of the defining meridian. So the sun is
10.8 minutes late here relative to the clock, every single day, and
there is nothing periodic about it. It is just where the town is.

**Two: the equation of time.** The sun itself is not a good clock. The
Earth's orbit is an ellipse, so the planet speeds up and slows down
through the year, and the axis is tilted 23.4 degrees, so the sun's
apparent motion is not parallel to the equator. The two effects together
make apparent solar time drift against clock time by an amount the Naval
Observatory says can reach as much as 16 minutes. It is called the
equation of time, and it runs through the same cycle every year.

You can watch both effects at once in the Naval Observatory's own
computed times for when the sun is highest at Silverdale:

| Date | Sun highest, Pacific Standard Time |
|---|---|
| 3 November 2026 | 11:54 |
| 21 December 2026 | 12:09 |
| 21 June 2026 | 12:13 |
| 11 February 2026 | 12:25 |

That is a 31 minute spread, at one fixed spot, with no clock changes
involved. Early November is when the sun runs furthest ahead of the
clock, mid February when it runs furthest behind.

**Three: daylight saving time, which is not an astronomical thing at
all.** US time zones and daylight saving are defined in US law, Title 15
of the US Code, and the Department of Transportation draws the
boundaries. Daylight saving begins on the second Sunday in March and
ends on the first Sunday in November, and the sun has never once been
informed. On 21 June 2026, the sun is highest over Silverdale at 12:13
standard time, which reads **13:13** on a clock that has been advanced
an hour.

Put the three together and the answer to "when is the sun highest here"
in midsummer is a quarter past one in the afternoon. If you have been
half-consciously assuming that noon means midday, this is the fact to
correct. It also explains why the hottest part of a summer afternoon
feels so late.

**What this means for using the sky as a clock.** You can get local
apparent solar time very well: watch the shadow, and when it is
shortest, that is local noon by definition. Converting that to the time
on your phone requires you to know your longitude, the date, and whether
the clocks have been changed, and to do about fifteen minutes' worth of
correction. A shadow will not give you civil time to better than about
half an hour unless you do the arithmetic properly.

Which is fine, because that is usually all you need. Knowing you have
four hours of daylight left is what matters. Knowing whether it is 16:05
or 16:22 is not.

## Season and date from the sun

This is the oldest use of the sky there is, older than writing, and you
can rebuild it with two sticks.

Watch the table of sunrise azimuths again. At Silverdale the sun comes
up at azimuth 52.7 on the June solstice and azimuth 125.0 on the
December solstice. That is 72.3 degrees of horizon, and the rising point
walks steadily from one end to the other and back over the course of a
year.

So: stand in the same spot every clear morning and note where against
the skyline the sun first appears. Over weeks it moves visibly. That is
a calendar. The sun's rising point is a physical pointer sliding along a
scale that consists of your own horizon.

The two ends of the walk are the solstices, and this is where the word
comes from: the rising point slows, stops, and reverses. The Naval
Observatory's dates and times for 2026, in Pacific Standard Time, are:

| Event | Date and time (PST) |
|---|---|
| Perihelion, Earth closest to the sun | 3 January, 09:15 |
| March equinox | 20 March, 06:46 |
| June solstice | 21 June, 00:24 |
| Aphelion, Earth furthest from the sun | 6 July, 09:30 |
| September equinox | 22 September, 16:05 |
| December solstice | 21 December, 12:50 |

Look at the first and fourth rows, because they kill a common
misconception permanently. The Earth is **closest** to the sun in early
January, two weeks after midwinter, and **furthest** in early July.
Distance is not what makes seasons. The Naval Observatory puts the
difference at only about 3 percent, and the tilt of the axis swamps it
entirely. Seasons are about the angle the light arrives at, which is the
noon altitude column back in the direction section: 65.8 degrees in
June, 18.9 in December, for the same sun at almost the same distance.

### Building the oldest calendar there is

The working version takes two sticks and a year.

Plant one stick as your standing point, so you observe from exactly the
same place every time. It matters: move ten metres sideways and the
whole alignment shifts. Plant the second stick, or better find a
permanent horizon feature, in line with the rising sun on a day you can
name.

Then keep watching. The rising point walks north through spring, slows
through June, holds nearly still for about a week either side of the
solstice, and starts back. Mark the extremes when you find them, and
mark the two points where the rising sun is due east; those are your
equinoxes. Once you have four marks you have a year divided into
quarters, permanently, with no paper involved.

There is a second version that uses shadow length rather than rising
point, and NOAA publishes a school lesson built on it. On either
equinox, the angle between a vertical stick and the line from its top to
the tip of its shadow, measured at local solar noon, equals your
latitude. That is the same measurement Eratosthenes used around
240 BC to work out the size of the Earth, which NOAA recounts in
Learning Lesson: The Shadow Knows I: he compared noon shadows at
Alexandria and Syene, found the angle difference was 7.2 degrees or one
fiftieth of a circle, and multiplied the 5,000 stadia between the cities
by fifty. NOAA converts his 250,000 stadia to 45,984 kilometres against a
true polar circumference of 40,009, so he landed about 15 percent high.
The uncertainty there is the length of a stadion, not his geometry, and
the geometry is the part worth copying.

Both versions matter for the same practical reason. A horizon calendar
and a shadow calendar both keep working when you have lost track of the
date, and knowing the date is the input to the entire growing calendar.
Your last spring frost here averages 29 March and your first autumn
frost averages 14 November; those numbers are useless if you do not know
what day it is.
[The Growing Calendar](the_growing_calendar.md) is what you do with the
answer.

## Latitude from the sun at noon

Polaris is the easy way, and it only works at night, in clear weather,
in the northern hemisphere. The sun works in daylight and works
everywhere, at the cost of one lookup.

The formula, for an observer north of the sun's track, which at
Silverdale is every day of the year:

```
latitude  =  90 degrees  -  (sun's altitude at its highest)  +  (sun's declination that day)
```

The sun's declination is how far north or south of the celestial equator
the sun is on that date. It runs from +23.44 degrees at the June
solstice to -23.44 at the December solstice, and it is the only thing in
the formula you cannot measure yourself with a stick.

Work it both ways with the Naval Observatory's numbers for Silverdale.

**December solstice 2026.** Sun's highest altitude 18.918 degrees,
declination -23.437.

```
90  -  18.918  +  (-23.437)  =  47.645
```

**June solstice 2026.** Sun's highest altitude 65.792 degrees,
declination +23.437.

```
90  -  65.792  +  23.437  =  47.645
```

The true latitude is 47.6445. Both answers land within a ten thousandth
of a degree, from opposite ends of the year, using the same formula.
Notice also that the two noon altitudes differ by 46.874 degrees, which
is exactly twice the 23.437 obliquity. The geometry is not
approximately consistent. It is consistent.

In the field you would not get four decimal places, because you are
measuring the sun's altitude with a stick and a protractor rather than
computing it. But an altitude good to a degree gives a latitude good to
a degree, which is the same accuracy Polaris gives you, in daylight.

Two cautions on the measurement itself. Never look at the sun to sight
it; measure its shadow and work back. And the altitude you want is of
the sun's centre, while what a shadow gives you is the edge, so there is
a quarter of a degree of slop built in before you start.

### The declination table question

You need the sun's declination for the day, and you cannot get it from
the sky. It has to come from somewhere.

The honest answer is that a printed table is the traditional source and
it is one of the few genuinely irreplaceable pieces of paper in
navigation. In the United States the canonical one is The Nautical
Almanac, which tabulates declination and Greenwich hour angle for the
sun, moon, planets and navigational stars, every hour of every day of
the year.

**Check the terms before you assume it is free, because it is not
straightforwardly a US government work.** The US Nautical Almanac Office
at the Naval Observatory has published it for more than 150 years, but
the Observatory states plainly that the book is produced in
collaboration with His Majesty's Nautical Almanac Office in the UK, and
that "That office maintains the copyright on the material it produces."
So the almanac is a joint US/UK product with UK Crown copyright over the
UK contribution. It is sold through the Government Publishing Office and
held by Federal Depository Libraries, and it cannot simply be reprinted
inside something like this guide.

What is unambiguously free is the Naval Observatory's own online data.
Its Celestial Navigation Data service will compute the sun's
declination, altitude and azimuth, plus the same for the moon, the
planets, the 57 navigational stars and Polaris, for any position and
time you give it. Every celestial number in this guide came from it. As
a work of the US federal government it is public domain under
17 U.S.C. 105.

https://aa.usno.navy.mil/data/celnav

Which leaves you three options, and they sit in a sensible order:

- **Buy the almanac.** The real answer if you are going to sea, and the
  only one that works with no power and no network.
- **Print a year of declinations** from the Naval Observatory's service
  before you need them, and keep the paper with your map. This is cheap,
  legal and the sensible preparation for anyone on land.
- **Use the rough rule.** The sun's declination is zero at the
  equinoxes, +23.44 at the June solstice and -23.44 at the December
  solstice, and in between it follows a shape close to a sine wave. That
  approximation is **a rule of thumb**, it is worst near the equinoxes
  where declination changes fastest, and it will not give you better
  than a degree or two. Which, again, may be all you need.

## Longitude, which is a clock problem

Here is the part that is usually fudged, so it is going to be said
plainly.

**You cannot determine your longitude from the sky without an accurate
clock.** Not with more skill, not with better tables, not with a clever
trick. The information is not in the sky.

The reason is that the Earth is a sphere spinning on its axis, and it
has no natural east-west landmark. Latitude has one: the pole is a real
physical direction fixed by the rotation, and it sits at a measurable
height in your sky. Longitude has nothing like it. Every meridian looks
exactly like every other meridian. Greenwich is zero because somebody
decided it was.

What longitude actually measures is a time difference. When the sun is
highest over you, it is noon where you are. If at that moment you know
what time it is at Greenwich, the gap between the two is your longitude,
because the Earth turns 15 degrees of longitude per hour. The Naval
Observatory states the same relationship from the other side: solar time
shifts by 4 minutes for each degree of longitude. Fifteen degrees per
hour and four minutes per degree are the same fact written twice.

So the instrument you need is a clock that still reads Greenwich time
correctly after weeks at sea, and until the eighteenth century no such
thing existed. NIST tells the clock half of this story, and only that
half: John Harrison, a carpenter and self-taught clockmaker, built a
marine chronometer by 1761 that kept time on a rolling ship to about one
fifth of a second a day, roughly as well as a pendulum clock did on
land, and about ten times better than the prize required. The prize was
worth more than ten million dollars in today's money. The rest of the
story is context NIST does not supply and this guide does not cite it
for: the prize was created by the Longitude Act of 1714, and the reason
it was a clock prize rather than a sky prize is that getting latitude
from the sky was already routine. NIST puts the
prize at over ten million dollars in today's money.

That is not a colourful historical aside. It is the measure of how hard
the problem is. The entire difficulty of eighteenth century navigation
was compressed into building a clock, because everything else was
already solved.

Work out what accuracy buys you, at this latitude, and you will see why.
One minute of longitude is one nautical mile at the equator, and NOAA
notes that the nautical mile was defined as one minute of latitude and
fixed at exactly 1.852 kilometres in 1929. At Silverdale's latitude the
meridians have converged, so a minute of longitude is 1.25 kilometres.
Four seconds of clock error is one minute of longitude. So:

- 1 second of clock error puts you about 310 metres east or west here.
- 1 minute of clock error puts you about 19 kilometres out.
- An hour puts you 15 degrees out, which is a whole time zone.

The good news for anyone reading this today is that the problem is
solved and the solution is in your pocket. Any phone, any GPS receiver,
any radio-set watch carries Greenwich time to far better than a second.
NIST makes the same point about modern navigation: GPS is a
timing system before it is a positioning system, and without atomic
clocks the errors would build up fast enough to make it useless.

The practical upshot is a division of labour worth remembering. **The
sky gives you latitude, direction and season for free, with no
equipment. Longitude costs you a clock.** A watch that is still right
after a week without a network is a navigation instrument, and it is the
one piece of kit that turns everything else in this guide into a
position instead of a line.

## What the moon tells you

The moon is the least useful of the three for direction and the most
useful for two other things.

**What it tells you about the date.** The Naval Observatory gives the
cycle as averaging 29.5 days from new moon to new moon, and defines the
four principal phases by the angle between the moon and the sun: new
moon when they are close together in the sky, full moon when they are
nearly opposite, and the two quarters when they are about 90 degrees
apart. A half-lit moon is a quarter moon and you are about a week from
either side of new. That gives you the date to within a couple of days
from a single glance, indefinitely, with no calendar.

It also predicts itself, though less precisely than the arithmetic looks.
A 29.5 day cycle spread over 24 hours gives about 49 minutes later each
night, which is arithmetic on the Observatory's figure rather than a
number it prints, and it is an average over the whole Earth and the whole
year. At this latitude the real delay swings from about 20 minutes near
the September equinox, which is the harvest moon and is the very example
used below, to about 70 minutes near the March equinox. The cause is the
angle the ecliptic makes with the horizon, so it is set by season and by
latitude, not by where you are in the month. The direction of the shift
is the reliable half: tomorrow's moon is later than tonight's.

**What it tells you about direction, honestly: not much, with one clean
exception.** Because a full moon is opposite the sun, it does what the
sun does, twelve hours out of step. It rises about when the sun sets and
is highest about local midnight, and at its highest it is due south, the
same as the noon sun.

Both halves check out for Silverdale on the full moon of
26 September 2026. The Naval Observatory has the moon rising at 17:46
and the sun setting at 18:00, fourteen minutes apart. For the second
half you have to catch the right minute, and it is worth seeing why. The
Observatory puts the moon's upper transit at 00:27 the next morning, and
at that instant it stood at azimuth 180.1, altitude 50.6, which is a
seventh of a degree off due south. By 00:50 it had swung to 188.8. An
earlier draft of this guide sampled 00:50 and offered the resulting nine
degrees as the confirmation, which proves nothing: sample any minute but
the transit and you get an off-south answer however right the rule is.

Away from full, the moon is a poor compass. Its orbit is tilted to the
Earth's, so its rising point wanders across a wider band of horizon than
the sun's and on a cycle that has nothing to do with the year. Most of
the folk rules for reading direction off a crescent are unreliable at
this latitude. If the moon is up and so is Polaris, use Polaris.

**What it tells you about the water, which is the real payoff here.**
The moon drives the tide, and the phase tells you which kind of tide you
are in. NOAA's explanation: when the sun, moon and Earth line up, at new
and full moon, the solar tide adds to the lunar tide and you get
extra-high highs and very low lows, called spring tides. When the sun
and moon are at right angles, at the quarters, the solar tide partially
cancels the lunar one and you get moderate neap tides. Two sets of each
per lunar month.

On a shoreline like Dyes Inlet that is not trivia. A full moon means the
biggest range of the fortnight, the lowest low water for clam digging
and the highest high water for everything you left on the beach. But
knowing the phase tells you the *kind* of tide, not the *time* or the
*height* of it, and the difference between those matters enough to be
dangerous. [Reading the Tide](reading_the_tide.md) has the actual
numbers, the published predictions, and the ways this shoreline kills
people.

## The honest limits

Four of them, and none is a small print item.

**Cloud.** This is the big one here and it is not close. Silverdale
averages 1,446 millimetres of precipitation a year, just under 80
percent of which falls between October and March, and about 102 days a
year carry 0.10 inches or more. December alone brings 252 millimetres.
Any technique in this guide that needs to see the sky is a technique
that fails for most of a Puget Sound winter, which is exactly the season
you are most likely to be out in the dark and cold and needing to know
which way is north. Plan on the assumption that the sky will not be
available when you most want it.

**Precision.** Everything here is degrees, not metres. Polaris gives you
latitude to about half a degree, which is 50 or 60 kilometres on the
ground. A hand-measured angle is worse. A shadow line is worse again in
December. This is navigation at the scale of "which valley" and "which
coast", not "which trailhead".

**Latitude and hemisphere.** Every number in this guide was computed for
47.6445 north. The sunrise azimuths, the noon altitudes, the day
lengths, the height of Polaris and the declination all change as you
move. The methods are universal; the figures are local. And south of the
equator, Polaris is simply gone, with no bright star marking the south
celestial pole, as NASA notes: southern navigators work from the
Southern Cross instead, which as established is not visible from here.

**A compass and a paper map are better tools, and you should carry
them.** That is not a grudging admission, it is the honest ranking. A
compass works in cloud, works in forest, works at any hour, costs
nothing to carry and needs no arithmetic once its declination is set. A
paper topographic map works when the phone is dead. The sky is the
backup that checks the compass and rescues you when the compass is lost
or lying, and it is the only one of the three that cannot run out or be
left at home. That is a real and valuable role. It is not a replacement.

## Practising where the sky is never cloudy

Which brings this back to the simulation, and to why this particular
skill sits differently in this project than most of the others.

HumanityOS draws its night sky from the HYG catalogue, which ships as
`data/stars.csv` and is loaded at runtime from the packed `data/stars.bin`
built from it. It holds 119,626 rows of which 119,625 are drawn, because
row zero is the Sun sitting at the origin and the loader discards it. The
constellation lines come from
`data/constellations.json`, which names all 88 IAU constellations. Two of
them, Cepheus and Hydrus, were missing until this guide went looking and
found they were not there; Cepheus was the awkward one, being the
circumpolar constellation next to Polaris on the Cassiopeia side, which
is the patch of sky this whole guide works in. Those figures are not
decoration painted on a dome. The catalogue is a compilation built from
the same professional astrometry that research catalogues use, and the
renderer resolves every constellation line to a real star in it: all 602
line segments, with no unresolved endpoints. That is now a test rather
than a claim, so a figure cannot quietly lose a line to a star we do not
have.

Which means the numbers cross over. The 5.374 degree gap between Dubhe
and Merak quoted at the top of this guide was computed from that file,
and it matches the 5.5 degrees NASA publishes for the real sky. Mintaka's
declination of -0.299, the fact that makes Orion's Belt a due-east
marker, is in that file. So is Acrux at -63.1, which is why you will not
find the Southern Cross over Silverdale in the game any more than you
will over Silverdale in the rain.

So a player can learn this indoors. Find the Dipper on a simulated sky,
walk the pointers to Polaris, learn the shape of Cassiopeia on the other
side, calibrate a hand against the pointer gap, and then walk outside on
the first clear night and do exactly the same thing with exactly the
same numbers. The practice transfers because the data is the same data.

One honest caveat, which is the same one from the precession section.
The catalogue holds J2000 positions, so the in-game Polaris sits 0.736
degrees from the pole rather than tonight's 0.619. The difference is a
tenth of a degree, far below what you can measure with a fist, and it
will not affect anything you practise. But it is there, and knowing your
instrument's error is most of what separates measuring from guessing.

That is the case for teaching this at all. Most survival skills are
practised badly or not at all because the real conditions are rare,
uncomfortable or dangerous. This one can be practised at three in the
afternoon, in a chair, on a cloudless simulated December night, as many
times as you like, and it costs nothing. And what you come away with
works on the actual sky, which has been running the same numbers for
longer than there have been people to read them.

## You own this when

- You can find Polaris from the Big Dipper in under a minute, and from
  Cassiopeia when the Dipper is in the trees.
- You know the height of Polaris above your horizon is your latitude,
  you can say why, and you can state the error and its cause rather than
  treating the method as exact.
- You have calibrated your own hand against a known angle in the sky and
  written the numbers down.
- You have a true north landmark picked out from where you live, and you
  found it from the sky rather than from a compass.
- You know your local magnetic declination, which direction to apply it,
  and that it changes with both place and year.
- You never assume that the shadow is shortest at noon, and you can name
  the three separate reasons it is not.
- You can get latitude from the noon sun as well as from Polaris, and
  you know the one number you have to look up to do it.
- You can say plainly that longitude requires an accurate clock, and you
  know roughly what a minute of clock error costs you on the ground.
- You know which of these methods fails in cloud, which is most of them,
  and you carry a compass and a paper map anyway.

The last one is the test of whether you have understood the rest. None
of this is a substitute for equipment. It is what lets you check your
equipment, and what is left when the equipment is gone.

## Sources

Grouped by what kind of authority each one is. Federal government
publications are listed first because they are works of the United
States government, are in the public domain under 17 U.S.C. 105, and can
be redistributed with this guide. The rest cannot, and are cited as the
authority with the facts restated here in our own words.

### United States government (public domain)

- United States Naval Observatory, Astronomical Applications Department.
  Celestial Navigation Data for Assumed Position and Time (the computed
  declination, altitude and azimuth of the sun, the moon and Polaris at
  47.6445 N, 122.6949 W for every date and time used in this guide:
  Polaris at declination 89.381 in 2026 and its five sampled altitudes
  and azimuths; the sun's noon altitudes of 65.792 and 18.918 and its
  noon azimuths of 180.18 and 179.997; the sunrise azimuths of 52.698,
  88.878 and 124.988 and the June sunset azimuth of 307.465; the full
  moon at altitude 50.362 and azimuth 188.800 on 27 September 2026).
  https://aa.usno.navy.mil/data/celnav
  and the API endpoint used, for example
  https://aa.usno.navy.mil/api/celnav?date=2026-12-21&time=20:09&coords=47.6445,-122.6949
- United States Naval Observatory. Complete Sun and Moon Data for One
  Day (sunrise, sunset and upper transit times at Silverdale for the
  solstices, the September equinox, 3 November and 11 February; moonrise
  on the 26 September 2026 full moon).
  https://aa.usno.navy.mil/api/rstt/oneday?date=2026-06-21&coords=47.6445,-122.6949&tz=-8&dst=false
- United States Naval Observatory. Earth's Seasons and Apsides (the 2026
  equinox, solstice, perihelion and aphelion dates and times).
  https://aa.usno.navy.mil/api/seasons?year=2026&tz=-8&dst=false
- United States Naval Observatory. Phases of the Moon (the 2026 full
  moon dates, used to pick the observation night).
  https://aa.usno.navy.mil/api/moon/phases/year?year=2026
- United States Naval Observatory. The Equation of Time (the definition
  as apparent solar time minus mean solar time; that it can reach as
  much as 16 minutes; the obliquity and eccentricity causes; and the
  rule that mean solar time runs 4 minutes later per degree of longitude
  west of the zone's defining meridian).
  https://aa.usno.navy.mil/faq/eqtime
- United States Naval Observatory. The Seasons and the Earth's Orbit
  (the 23.4 degree obliquity as the cause of the seasons; perihelion in
  early January about two weeks after the December solstice; the
  perihelion-to-aphelion distance difference of only about 3 percent).
  https://aa.usno.navy.mil/faq/seasons_orbit
- United States Naval Observatory. Phases of the Moon and Percent of the
  Moon Illuminated (the four principal phases defined by the sun-moon
  angle; new moon close to the sun, full moon nearly opposite, quarters
  about 90 degrees apart; the cycle averaging 29.5 days).
  https://aa.usno.navy.mil/faq/moon_phases
- United States Naval Observatory. Daylight Saving Time (daylight saving
  and the time zones codified in US Code Title 15 Chapter 6 Subchapter
  IX; the second Sunday in March to first Sunday in November schedule
  set by the Energy Policy Act of 2005).
  https://aa.usno.navy.mil/faq/daylight_time
- United States Naval Observatory. U.S. Time Zones (time zones defined
  in law, with boundaries set by the Department of Transportation).
  https://aa.usno.navy.mil/faq/us_tzones
- United States Naval Observatory. Rise, Set, and Twilight Definitions
  (sunrise and sunset defined at the upper edge of the disk, computed at
  50 arcminutes below the horizon, being 16 arcminutes of solar radius
  plus 34 arcminutes of refraction; civil, nautical and astronomical
  twilight at 6, 12 and 18 degrees).
  https://aa.usno.navy.mil/faq/RST_defs
- United States Naval Observatory. Celestial Navigation Resources (the
  Nautical Almanac and the Air Almanac as the publications containing
  the data needed to practise celestial navigation at sea and in the
  air). https://aa.usno.navy.mil/faq/celnav
- United States Naval Observatory. The Nautical Almanac (that the US
  Nautical Almanac Office has published it for more than 150 years; its
  contents, tabulated hourly to 0.1 arcminute; and the copyright
  statement quoted in this guide, that the book "is produced in
  collaboration with His Majesty's Nautical Almanac Office in the UK"
  and "That office maintains the copyright on the material it
  produces"). https://aa.usno.navy.mil/publications/na
- United States Naval Observatory. Ordering Information (the almanacs
  sold through the Government Publishing Office and, in the UK, through
  the Admiralty; copies held by Federal Depository Libraries).
  https://aa.usno.navy.mil/publications/ord_info
- NOAA National Centers for Environmental Information. Magnetic Field
  Calculator, World Magnetic Model WMM-2025 (declination at 47.6445 N,
  122.6949 W on 15 September 2026 of 14.950 degrees east, with an
  uncertainty of 0.385 degrees and an annual change of -0.124 degrees
  per year).
  https://www.ngdc.noaa.gov/geomag/calculators/magcalc.shtml (the keyed
  JSON endpoint behind this page returns HTTP 400 without a free NCEI
  API key, so the reader-facing calculator is the link given here)
  The interactive version, for looking up any other place, is at
  https://www.ngdc.noaa.gov/geomag/calculators/magcalc.shtml
- NOAA NCEI. The World Magnetic Model (declination as the angle between
  magnetic and geographic north; a compass pointing at the local
  magnetic force rather than at the pole; the model updated every five
  years because the field changes; WMM2025 released 17 December 2024 and
  expiring 31 December 2029).
  https://www.ncei.noaa.gov/products/world-magnetic-model
- NOAA NCEI. World Magnetic Model Accuracy, Limitations and Error Model
  (the declination uncertainty formula and its dependence on horizontal
  field strength; the polar blackout zones below 2000 nT where compasses
  should not be relied on; the caution zones between 2000 and 6000 nT).
  https://www.ncei.noaa.gov/products/world-magnetic-model/accuracy-limitations-error-model
- NOAA National Weather Service, JetStream. Learning Lesson: The Shadow
  Knows II (the equinox noon shadow angle equalling the observer's
  latitude; Eratosthenes at Alexandria and Syene, the 7.2 degree angle
  and the one fiftieth of a circle; the polar circumference of
  24,860 miles or 40,009 kilometres and the equatorial figure of
  24,902.4 miles or 40,076.5 kilometres).
  https://www.noaa.gov/jetstream/global/learning-lesson-shadow-knows-ii
- NOAA National Weather Service, JetStream. Learning Lesson: The Shadow
  Knows I (shadow length changing with season because of axial tilt
  rather than distance to the sun; the analemma as the figure-eight the
  sun traces at a fixed clock time through the year, and the instruction
  to ignore daylight saving when building one).
  https://www.noaa.gov/jetstream/global/learning-lesson-shadow-knows-i
- NOAA National Ocean Service. What is the difference between a nautical
  mile and a knot? (one nautical mile equals one minute of latitude; set
  at exactly 1.852 kilometres in 1929; equal to 1.1508 statute miles).
  https://oceanservice.noaa.gov/facts/nautical-mile-knot.html
- NOAA National Ocean Service Education. Tidal Variations: The Influence
  of Position and Distance (spring tides at new and full moon when sun,
  moon and Earth align and the solar tide adds to the lunar one; neap
  tides at the quarters when they are at right angles; two of each per
  lunar month).
  https://oceanservice.noaa.gov/education/tutorial_tides/tides06_variations.html
- NOAA Global Monitoring Laboratory. Solar Calculator Details (the
  0.833 degrees of atmospheric refraction assumed at sunrise and sunset;
  accuracy within a minute for locations between plus and minus 72
  degrees of latitude). https://gml.noaa.gov/grad/solcalc/calcdetails.html
- National Institute of Standards and Technology. A Walk Through Time: A
  Revolution in Timekeeping (the longitude problem and why a shipboard
  clock was the missing piece; the Longitude Act; John Harrison as a
  carpenter and self-taught clockmaker; the 1761 marine chronometer
  keeping time to about one fifth of a second a day on a rolling ship,
  ten times better than the prize required; determining longitude to
  within half a degree).
  https://www.nist.gov/pml/walk-through-time-revolution-timekeeping
- National Institute of Standards and Technology. Knowing Where We Are
  (Harrison's chronometer at one fifth of a second per day; GPS as a
  timing system, needing signals from at least four satellites, and
  unusable without atomic clocks because errors would build up too fast).
  https://www.nist.gov/atomic-clocks/a-technology-powerhouse/knowing-where-we-are
- NASA Science. What is the North Star and How Do You Find It? (the two
  stars on the end of the Big Dipper's cup pointing to Polaris at the
  tip of the Little Dipper's handle; Polaris tracing a very small circle
  over 24 hours because it is close to the celestial pole; the axis
  wobbling over about 26,000 years; Vega as the pole star about 14,000
  years ago and again in about 12,000 years; the southern hemisphere
  having no bright pole star and using the Southern Cross instead).
  https://science.nasa.gov/solar-system/what-is-the-north-star-and-how-do-you-find-it/
- NASA Science. Basics of Space Flight, Chapter 2: Reference Systems
  (declination as the celestial equivalent of latitude and right
  ascension as the equivalent of longitude; the celestial sphere's pole
  and equatorial plane coincident with the Earth's).
  https://science.nasa.gov/learn/basics-of-space-flight/chapter2-2/
- United States Geological Survey. What Direction Am I Facing? (adding
  or subtracting local declination on a fixed compass, against setting
  the declination once on an adjustable one and reading true bearings
  directly).
  https://www.usgs.gov/educational-resources/what-direction-am-i-facing

### Cited as the authority, facts restated

- NASA Night Sky Network, Measure the Night Sky. The hand angles used in
  this guide: a little finger at about 1 degree, three middle fingers at
  about 5.5 degrees, a spread hand at about 25 degrees, and a full moon
  at about half a degree. Also the statement that the Big Dipper's
  pointer stars are about 5.5 degrees apart, which this guide
  cross-checks against the project's own star catalogue. The Night Sky
  Network carries NASA branding but states that it is managed by the
  Astronomical Society of the Pacific, a nonprofit, so it is not
  straightforwardly a federal work and its text is not reproduced here.
  https://nightsky.jpl.nasa.gov/news/236/

### Project data

- `data/stars.csv`. The 119,626-row HYG catalogue the 3D sky renderer
  draws from. Source of every star position quoted here: Polaris at
  declination 89.2641 for epoch J2000, Dubhe at 61.7510, Merak at
  56.3824, Alkaid at 49.3133, Schedar at 56.5373, Kochab at 74.1555,
  Mintaka at -0.2991 and Acrux at -63.0991. The angular separations
  computed from it, 5.374 degrees Dubhe to Merak, 28.707 Dubhe to
  Polaris, 34.070 Merak to Polaris, 32.813 Schedar to Polaris and 16.578
  Kochab to Polaris, are arithmetic done here on those positions.
- `data/constellations.json`. All 88 constellations and their line
  figures, including the notes that Dubhe and Merak point to Polaris,
  that Polaris lies within 1 degree of the celestial north pole, that
  Vega held the job about 12,000 years ago, and that Thuban was the pole
  star around 2700 BC. Also the source for Crux being in the game's sky
  at all.
- `data/locales/silverdale_wa/locale.json`. The latitude and longitude,
  47.6445 north and 122.6949 west, that every computed figure in this
  guide was requested for.
- `data/locales/silverdale_wa/climate.json`. The annual precipitation of
  1,446 millimetres, the December figure of 252 millimetres, the roughly
  80 percent of the year's water falling October through March, the
  about 102 days a year with 0.10 inches or more, the average last
  spring frost of 29 March and first autumn frost of 14 November, and
  the daylight hours that the solstice day lengths here agree with. Its
  own provenance is NOAA NCEI and the same Naval Observatory service
  used above.

### Inside this project

- [Reading the Tide](reading_the_tide.md), for what the moon's phase
  does and does not tell you about the water, and for the published
  predictions this guide deliberately does not try to replace.
- [The Growing Calendar](the_growing_calendar.md), for what knowing the
  date is actually for here.
- [Units and Converting Them](units_and_converting_them.md), for angles,
  the nautical mile, and the habit of carrying units through arithmetic
  so a wrong conversion announces itself.
- [Where You Are: Silverdale, Washington](../locale/silverdale_wa.md),
  for the rest of the setting these numbers belong to.

### Labelled in the text as rules of thumb, not sourced

- **A closed fist at about 10 degrees.** The Night Sky Network publishes
  the finger and hand-span angles but not the fist, and fist size varies
  between people more than finger width does. Calibrate your own against
  the pointer gap.
- **Averaging two Polaris readings twelve hours apart** to cancel its
  offset from the pole. This follows directly from the geometry and is
  demonstrated on the sampled values in this guide, but it is not a
  published procedure.
- **The shadow-tip two-mark method** for an east-west line. It is in
  every survival manual, and no federal publication of it was found
  while writing this guide, so it is presented as an approximation with
  its error explained: the shadow tip traces a curve rather than a
  straight line on every day except the equinoxes, and the error is
  worst near the solstices and at high latitudes, which describes
  Silverdale in December.
- **Approximating the sun's declination as a sine wave** between plus
  and minus 23.44 degrees. Useful to a degree or two, worst near the
  equinoxes where declination changes fastest, and not a substitute for
  a table.
- **The moon rising about 49 minutes later each night.** Arithmetic on
  the Naval Observatory's 29.5 day cycle rather than a figure it prints,
  and it varies through the month.
- **111 kilometres per degree of latitude.** Arithmetic on NOAA's polar
  circumference of 40,009 kilometres, and it cross-checks against 60
  nautical miles per degree at 1.852 kilometres each, giving 111.12.
  Both are averages; the Earth is not a perfect sphere.
- **That long arms tend to come with large hands**, which is why
  measuring angles at arm's length works across different people. A
  general observation about human proportion, not a published constant,
  and the reason the guide tells you to calibrate your own hand anyway.
