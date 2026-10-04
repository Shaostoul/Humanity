# Coordinates

In a town, an address does the job. The Army's map reading manual puts
it simply: "In a city, it is quite simple to find a location; the
streets are named and the buildings have numbers." Away from the
streets, and for anything that has no address at all, you need
something else: the spring at the bottom of the field, the place a
water line crosses the drive, the trailhead where a friend is waiting
with a twisted ankle. That something is a coordinate, a short string of
numbers that names one spot on Earth.

A coordinate is only useful if the person who reads it ends up where
you meant. Most of the skill is not the numbers themselves but the
habits that stop them going wrong: the letter or minus sign that says
which half of the world, the order of the two numbers, the format, the
reference they are measured on, and an honest idea of how accurate they
are. This guide covers latitude and longitude, the grid references used
on topographic maps and by emergency planners, and how to give and
follow a position that another person can actually find.

It draws on NOAA's National Ocean Service and the US Geological Survey
(USGS) for what latitude and longitude are and how far a degree is, on
the Army's *Map Reading and Land Navigation* (FM 3-25.26, 2001) for
geographic and grid coordinates, on the Federal Geographic Data
Committee (FGDC) for the US National Grid, on GPS.gov for how accurate a
phone's position is, and on the US Forest Service and the Pipeline and
Hazardous Materials Safety Administration (PHMSA) for safety. All are
United States government publications. Where something is general
practice, arithmetic or our own reading, the text says so.

[Reading a Map](reading_a_map.md) comes before this guide, and
[Knowing Which Way Is North](knowing_which_way_is_north.md) beside it.
[The Sky as an Instrument](the_sky_as_an_instrument.md) explains how
latitude can be measured from the stars, and why longitude needs a
clock.

## First: the mistakes that send people to the wrong place

A wrong coordinate is worse than none, because it looks exact. These
are the mistakes that matter most when someone is relying on you.

- **A missing letter or minus sign.** The Army manual says that because
  a latitude can have the same number north or south of the equator,
  "the direction N or S must always be given", and likewise "The
  direction E or W must always be given" for longitude. In the
  signed-number style a phone uses, the minus sign does that job:
  negative latitude is south, negative longitude is west (general
  practice). Drop the minus from the longitude of a point in Washington
  State, 47.6448, -122.6952, and you have named a point about 7,700
  kilometres away in Asia (arithmetic).
- **The two numbers in the wrong order.** Latitude is usually written
  first and longitude second (general practice), but not every app or
  document does it the same way. Writing the letters, N and W, makes the
  order impossible to mistake. A "latitude" above 90 is a sign the
  numbers have been swapped, because, as the manual says, latitude runs
  only to 90 degrees north and south (our check).
- **Mixing formats.** 47 degrees 39 minutes is 47.65 degrees, not 47.39.
  Mistaking one for the other moves a point about 29 kilometres
  (arithmetic). Say which format you are using.
- **A different datum.** The same point can carry different numbers on
  different reference systems, called datums. The USGS says that in the
  48 conterminous states the shift between the North American Datum of
  1927 and that of 1983 is "in the range of 10-100 ground meters" for
  latitude and longitude, and around 200 metres for grid coordinates.
  The Army manual says grid coordinates of the same point on different
  datums "may differ as much as 900 meters", and tells map readers to
  check the datum note on every map.
- **Believing the phone too much.** GPS.gov says that smartphones "are
  typically accurate to within a 4.9 m (16 ft.) radius under open sky",
  and that "their accuracy worsens near buildings, bridges, and trees."
  Indoors and underground it is worse again.
- **Counting on a phone at all.** The Forest Service warns that a GPS
  device sometimes does not get a signal or its battery fails, and that
  "Cell phones also likely will not work because of a lack of signal."
  Carry a map and compass, and be able to describe where you are in
  words.
- **Digging by coordinate.** No coordinate, from a phone or a plan, is
  good enough to find a buried pipe or cable. PHMSA says "Don't assume
  that you know what's below." Call 811 before every digging project,
  even small ones; the companies whose lines could be affected must mark
  them with flags or paint, and there is no cost to you (PHMSA). They
  mark only their own lines, not the ones you or an earlier owner laid
  ([Square, Level and Plumb](square_level_and_plumb.md) covers finding
  those).

## Latitude and longitude

NOAA's National Ocean Service defines the two plainly: "Latitude
measures the distance north or south of the equator", and "Longitude
measures distance east or west of the prime meridian."

- **Latitude** starts at 0 degrees at the equator and rises to 90
  degrees at each pole, north or south. Lines of latitude, called
  parallels, run east and west around the globe, parallel to the
  equator.
- **Longitude** starts at 0 degrees at the prime meridian, which runs
  through Greenwich in England, and is counted up to 180 degrees east
  and 180 degrees west. Lines of longitude, called meridians, run from
  pole to pole.

The Army manual explains the units: each circle is divided into 360
degrees, each degree into 60 minutes, and each minute into 60 seconds.

A small detail that teaches a big lesson: NOAA notes that if you stand at
the prime meridian markers in Greenwich with a GPS receiver, you have to
walk 102 metres east before it shows 0 degrees longitude. The line on
the ground and the line in the satellites' reference were drawn by
different methods. That is the datum problem in miniature.

### How far is a degree?

A degree of latitude is always about the same length. NOAA says "Each
degree of latitude covers about 111 kilometers on the Earth's surface",
and that a second of latitude covers only about 30.7 metres.

A degree of longitude is not. The meridians meet at the poles, so the
distance between them shrinks as you go north or south. NOAA's figures:
at the equator one degree of longitude covers about 111 kilometres; by
60 degrees north or south it is down to 56 kilometres; at the poles it
is zero. The USGS gives the figures at 38 degrees north, a line through
Stockton, California and Charlottesville, Virginia: a degree of latitude
is about 69 miles, but a degree of longitude only 54.6 miles.

A good rule for the length of a degree of longitude is 111 kilometres
times the cosine of your latitude (arithmetic, and it matches both sets
of figures: at 60 degrees the cosine is one half, giving 55.5
kilometres). At 47.6 degrees north, the latitude of Silverdale,
Washington, it is about 75 kilometres.

### What each decimal place is worth

Phones and most software write coordinates as decimal degrees. From
NOAA's 111 kilometres a degree (arithmetic):

| Decimal places | Example | North-south | East-west at 47.6 degrees |
|---|---|---|---|
| 1 | 47.6 | about 11 km | about 7.5 km |
| 2 | 47.64 | about 1.1 km | about 750 m |
| 3 | 47.645 | about 111 m | about 75 m |
| 4 | 47.6448 | about 11 m | about 7.5 m |
| 5 | 47.64481 | about 1.1 m | about 75 cm |

Read that table against GPS.gov's figure for a phone, about 5 metres
under open sky. Four decimal places already match what a phone can do.
A fifth or sixth decimal place copied from a phone describes the
screen, not the ground. [Reading Numbers
Honestly](reading_numbers_honestly.md) calls this false precision; write
the digits you can stand behind (our reading).

### Three ways to write the same place

The same point can be written three ways (arithmetic):

| Format | Example |
|---|---|
| Degrees, minutes and seconds (DMS) | 47 degrees 38' 41.3" N, 122 degrees 41' 42.7" W |
| Degrees and decimal minutes (DDM) | 47 degrees 38.688' N, 122 degrees 41.712' W |
| Decimal degrees (DD) | 47.6448, -122.6952 |

To convert DMS to decimal degrees, divide the minutes by 60 and the
seconds by 3,600 and add them on. For the other direction, multiply the
decimal part by 60 to get minutes, and the decimal part of that by 60 to
get seconds (arithmetic).

**Worked example.** An old map gives a spring at 47 degrees 39' 07" N,
122 degrees 41' 20" W, and you want it in your phone.

- Latitude: 47 + 39/60 + 7/3,600 = 47 + 0.65 + 0.00194 = 47.65194.
- Longitude: 122 + 41/60 + 20/3,600 = 122.68889, west, so -122.68889.
- Write it to the accuracy the map supports, say 47.6519, -122.6889, and
  note the map's datum beside it.

### How the world keeps time by longitude

Longitude and time are tied together. NOAA explains that each hour of
difference between local noon and the time at Greenwich "equals 15
degrees of longitude", because the Earth turns 360 degrees in 24 hours.
That is why navigators once found longitude with an accurate clock, and
why [The Sky as an Instrument](the_sky_as_an_instrument.md) says you
cannot get longitude from the sky without one.

## Grid references

Topographic maps and emergency planners often use a grid instead of
latitude and longitude: a set of numbered squares in metres laid over
the map.

The USGS explains the main one: UTM, the Universal Transverse Mercator
grid, "consists of 60 zones, each 6-degrees of longitude in width",
numbered 1 to 60 eastward from 180 degrees longitude. The military uses
its own version, the Military Grid Reference System (MGRS). The USGS also
says that "One system is no more or less accurate than the other": they
are different ways to name the same point. The FGDC publishes the US
National Grid (USNG) as a federal standard, describing it as "an
alpha-numeric reference system that overlays the UTM coordinate
system"; its page records US emergency planners adopting it as a
common way to name places.

### Reading a grid reference

The FGDC's guide breaks a full US National Grid address into three
parts. Its example is the Washington Monument:

**18S UJ 23480647**

- **18S** is the grid zone, which makes the address unique anywhere on
  the planet.
- **UJ** names one 100 kilometre square within that zone.
- **23480647** is the grid coordinate inside that square.

The rule for the numbers, which the Army manual states the same way:
"always read right, then up". The first half of the digits is how far
east (right), and the second half how far north (up). So 23480647 splits
as 2348 east and 0647 north. Coordinates "are always given as an even
number of digits so you know where to separate the easting and northing
coordinates."

The more digits, the smaller the area named. The FGDC's figures:

| Digits | Example | Locates a point to a precision of |
|---|---|---|
| 4 | 2306 | 1,000 metres (a neighbourhood) |
| 6 | 234064 | 100 metres (a soccer field) |
| 8 | 23480647 | 10 metres (a modest home) |
| 10 | 2348306479 | 1 metre (a parking spot) |

The Army manual adds that grid coordinates are written as one
continuous number, without spaces, dashes or decimal points, and that it
normally reports locations to six digits, 100 metres, and targets to
eight.

Notice the trap between the two systems: latitude and longitude are
usually said north first, then east or west, while a grid reference is
always read right (east) first, then up (north). Saying which system you
are using prevents it (our note).

The FGDC's example also writes the datum after the address,
"(NAD 83)". That is a good habit to copy for any coordinate you write
down (our reading).

## Giving a position someone can find

When you pass a position to another person, whether a neighbour, a
search team or yourself in ten years' time, give them enough to check
it, not just enough to type it (general practice throughout, built on the
mistakes listed at the top):

1. **Start with words.** A description works even when a coordinate is
   wrong: "on the forest road, 200 metres past the gate, where the power
   line crosses". The Army manual starts its chapter on locating points
   from the address, and a description is the address of a place that
   has none.
2. **Say the system and format.** "Decimal degrees, from my phone", or
   "grid reference off the paper map".
3. **Give both numbers with their direction.** "Forty-seven point six
   four four eight north, one two two point six nine five two west." Say
   north and west aloud even if the screen shows a minus sign.
4. **Say how good it is.** "Phone, open sky, good to about 5 metres", or
   "read off a 1:24,000 map, good to about 100 metres".
5. **Name the datum if you know it,** especially for anything read off
   an old paper map.
6. **Have them read it back,** digit by digit.
7. **Stay where you said you are.** If you are waiting to be found, the
   Forest Service's advice is to stay put, and a position is only useful
   if you are still at it.

### Following a position someone gave you

1. **Check it is plausible** before you set off. Plot it on a map. Is it
   near where you expected, on the right side of the river, in the right
   country? A dropped minus sign or swapped numbers usually puts a point
   somewhere absurd.
2. **Check the format and datum** against your map or device.
3. **Use the description to confirm** you are in the right place when
   you arrive.

(All general practice.)

## Worked example: recording a spring on your land

An illustration with made-up numbers.

You find a spring in the woods and want to find it again, and to tell
the family where it is.

1. Stand at the spring under the most open sky you can find and let the
   phone settle. It reads 47.652131, -122.689044.
2. Keep four decimal places, the phone's real accuracy: **47.6521 N,
   122.6890 W**. That names a spot about 11 metres north-south by 7.5
   metres east-west at this latitude, close to the phone's 5 metre
   radius (arithmetic and GPS.gov).
3. Add the words: "Spring, 30 paces downhill from the old cedar, on the
   east side of the creek."
4. Note how you got it and when: "Phone, open sky under the alders, 3
   October 2026."
5. Write it into your records ([Keeping Records](keeping_records.md)),
   and on a copy of the property map.

If one day you lay a pipe from that spring, record where it runs the
same way, and also as tape measurements from two things that will not
move, such as the corners of a building: a phone's 5 metres is too
coarse to dig by, and 811 will not mark a line you laid yourself
(general practice). Before any digging, call 811 first (PHMSA).

## Know where coordinates stop

- **Property lines.** A coordinate from a phone or a map is not a
  boundary. Where your land ends is a question for the deed and a
  licensed land surveyor (general practice, as in [Reading a
  Map](reading_a_map.md)).
- **Buried lines.** Call 811 before digging; the marks on the ground are
  what count (PHMSA).
- **Boats and aircraft.** Navigation on water and in the air uses its
  own charts, conventions and training (general practice).
- **Emergencies.** If someone is hurt or missing, call the emergency
  services first and give them your location in words and numbers; do
  not wait until you have perfect coordinates (general practice).

## How the game models it

The game uses real latitude and longitude in a few places you can see,
and in more that you cannot.

- **Your home's coordinate.** On the Inventory page, under "You & your
  places", the Home card shows "Silverdale, WA" and the coordinate
  47.645, -122.695: decimal degrees, latitude first, a minus sign for
  west, three decimal places. By the table above, three places name a
  spot to within about 55 metres north-south and 37 metres east-west
  (half a step each way; arithmetic). The number comes from the stored
  value 47.6448, -122.6952 in the game's places data.
- **The map's footer.** On the Maps page, the Planet view's footer names
  the region and its centre to four decimal places, for example
  "Silverdale (47.6400, -122.6750)".
- **Metres per degree.** To lay the real map regions out on the ground,
  the game counts 110,540 metres to a degree of latitude, and 111,320
  metres times the cosine of the latitude to a degree of longitude: the
  same shrinking-longitude rule as this guide, with fixed numbers close
  to NOAA's 111 kilometres. The regions are built into the game's 3D
  Earth at their real latitude and longitude once the camera comes within
  40 kilometres of them.
- **Time by longitude.** The game's clock is the time at longitude 0,
  like Greenwich. A place's local sun time is one hour later for every
  15 degrees east of that, as NOAA describes, and solar panels, grow lights and
  crops at the home follow the sun of the home's longitude. At
  Silverdale, 122.7 degrees west, local noon falls at about 20:11 on that
  clock (arithmetic).

What the game does not model, so you do not learn it from the game:
there is no readout of your own position as you walk (the GPS Device in
the item list does nothing yet), no grid references, only one
reference system, so no datum differences, and no measurement error:
a position the game shows is never off the way a phone's is, and a real
one always is, a little.

## You own this when

- You always write N or S and E or W, or the minus sign, and you say it
  aloud when you read a coordinate to someone.
- You can tell a swapped pair at a glance, and you check a coordinate is
  plausible before you go.
- You can convert between degrees, minutes and seconds, decimal minutes
  and decimal degrees, and you never write 47 degrees 39 minutes as
  47.39.
- You know a degree of latitude is about 111 kilometres, a degree of
  longitude shrinks with the cosine of the latitude, and what each
  decimal place is worth.
- You keep only the digits your source can support, and you know a
  phone is good to about 5 metres in the open and worse among trees and
  buildings.
- You can read a grid reference right, then up, and say how big the
  square is from the number of digits.
- You note the datum, especially from an old map.
- You give a position with words as well as numbers, and have it read
  back.
- You call 811 before digging, whatever your records say.

## Sources

Grouped by what kind of authority each one is. Every source here is a
work of the United States federal government, and its own text is in
the public domain. Web pages were read on 3 October 2026.

### United States government (public domain)

- NOAA National Ocean Service. What is latitude?, last updated 23
  September 2026 ("Latitude measures the distance north or south of the
  equator"; 0 to 90 degrees; parallels; "Each degree of latitude covers
  about 111 kilometers on the Earth's surface"; a second of latitude
  about 30.7 metres).
  https://oceanservice.noaa.gov/facts/latitude.html
- NOAA National Ocean Service. What is longitude?, last updated 23
  September 2026 ("Longitude measures distance east or west of the prime
  meridian"; the prime meridian through Greenwich and the 102 metres to
  GPS zero; a degree of longitude about 111 kilometres at the equator, 56
  at 60 degrees and zero at the poles; each hour of difference "equals
  15 degrees of longitude").
  https://oceanservice.noaa.gov/facts/longitude.html
- US Geological Survey. How much distance does a degree, minute, and
  second cover on your maps?, updated 17 March 2023 (at 38 degrees north,
  a degree of latitude about 69 miles and of longitude 54.6 miles).
  https://www.usgs.gov/faqs/how-much-distance-does-a-degree-minute-and-second-cover-your-maps
- US Geological Survey. How large is the North American Datum of 1927
  (NAD 27) to NAD 83 shift?, updated 18 July 2025 ("in the range of
  10-100 ground meters" for latitude and longitude in the conterminous 48
  states, around 200 metres for UTM).
  https://www.usgs.gov/faqs/how-large-north-american-datum-1927-nad-27-nad-83-shift
- US Geological Survey. What does the term UTM mean? Is UTM better or
  more accurate than latitude/longitude?, updated 17 March 2023 (60 zones
  "each 6-degrees of longitude in width", numbered from 180 degrees
  eastward; MGRS as the military version; "One system is no more or less
  accurate than the other").
  https://www.usgs.gov/faqs/what-does-term-utm-mean-utm-better-or-more-accurate-latitudelongitude
- Headquarters, Department of the Army. *Map Reading and Land
  Navigation*, FM 3-25.26, 20 July 2001, approved for public release,
  distribution unlimited. Chapter 4 (the city address; latitude and
  longitude, degrees, minutes and seconds, "the direction N or S must
  always be given" and "The direction E or W must always be given";
  grid coordinates read RIGHT and UP, more digits for more precision,
  written as one continuous number with an even number of digits,
  normally six digits for locations and eight for targets) and chapter 3
  (the horizontal datum note, to be checked on every map, and grid
  coordinates of the same point on different datums that "may differ as
  much as 900 meters"). The edition is the one cited in [Reading a
  Map](reading_a_map.md).
  https://archive.org/download/MManuals/Fm3-25.26MapReadingAndLandNavigation.pdf
- Federal Geographic Data Committee. How to Read a United States
  National Grid (USNG) Spatial Address, undated (grid zone, 100,000
  metre square and grid coordinates; the Washington Monument at 18S UJ
  23480647; "always read right, then up"; an even number of digits; 4,
  6, 8 and 10 digits for 1,000, 100, 10 and 1 metre; the datum written
  after the address). The FGDC's United States National Grid page
  records the standard (FGDC-STD-011-2001) and its adoption by
  emergency planners.
  https://www.fgdc.gov/usng/how-to-read-usng/index_html
  https://www.fgdc.gov/usng
- GPS.gov (National Coordination Office for Space-Based Positioning,
  Navigation, and Timing). GPS Accuracy, undated (smartphones "typically
  accurate to within a 4.9 m (16 ft.) radius under open sky", worse
  "near buildings, bridges, and trees"; signal blockage, indoor and
  underground use and reflected signals as common causes of error). The
  page credits its smartphone figure to a paper published through the
  Institute of Navigation.
  https://www.gps.gov/gps-accuracy
- USDA Forest Service. If You Get Lost, page last modified 4 December
  2023 (GPS devices that lose signal or battery; "Cell phones also
  likely will not work because of a lack of signal"; tell someone the
  details of your trip; stay put).
  https://www.fs.usda.gov/visit/know-before-you-go/if-you-get-lost
- Pipeline and Hazardous Materials Safety Administration. Call Before
  You Dig!, undated (call 811 before every digging project, even
  planting trees or shrubs; lines marked with flags or paint; no cost;
  "Don't assume that you know what's below.").
  https://primis.phmsa.dot.gov/stakeholder-comms/cbyd/

### Inside this project

- The Home card's coordinate: `coordinate` in `data/places/seed.json`,
  shown to three decimals by `draw_container` in
  `src/gui/pages/inventory.rs`.
- The Planet view's footer: `draw_planet_view` in
  `src/gui/pages/cosmos.rs`; the two regions, `data/maps/regions/`.
- Metres per degree: `M_PER_DEG_LAT` and `M_PER_DEG_LON_EQUATOR` in
  `src/terrain/osm_region.rs`; the regions built into the 3D Earth
  within 40 kilometres (`BUILD_RANGE_M` in `src/engine/region_meshes.rs`).
- Time by longitude: `local_hour` and `solar_hour_at` in
  `src/systems/time.rs`, used by the solar panels
  (`src/systems/solar.rs`), the grow lights and the crops.
- The GPS Device item, `gps_device_0` in `data/items.csv`, which no game
  code uses yet.
- [Reading a Map](reading_a_map.md), [Knowing Which Way Is
  North](knowing_which_way_is_north.md), [The Sky as an
  Instrument](the_sky_as_an_instrument.md), [Reading Numbers
  Honestly](reading_numbers_honestly.md) and [Keeping
  Records](keeping_records.md).

### Labelled in the text as general practice, arithmetic or our reading, not sourced

- That a minus sign stands for south or west in signed decimal degrees,
  and that latitude is usually written first.
- The distance a dropped minus sign moves a Silverdale point, the 29
  kilometre format error, the cosine rule, the decimal-place table, the
  format conversions and the game's 55 by 37 metre rounding: arithmetic
  from the sources' figures.
- That a latitude above 90 shows a swapped pair.
- Keeping only the digits a source supports, as a reading of GPS.gov's
  figure and of [Reading Numbers Honestly](reading_numbers_honestly.md).
- The difference in order between latitude and longitude and a grid
  reference, and writing the datum after every coordinate.
- The steps for giving and following a position, and the worked example
  (made-up numbers).
- Recording your own buried lines by tape measurements from fixed
  points as well as by coordinate.
- The limits: surveyors for boundaries, training for boats and
  aircraft, and calling the emergency services first.
