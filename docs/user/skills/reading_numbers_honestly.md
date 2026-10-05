# Reading Numbers Honestly

Numbers arrive with claims attached. The new feed "raises egg production
by 20 percent". This year's potato harvest was "down". A heat pump "saves
families $150 a month". A water test "came back fine". A neighbour's
compost "doubled" his tomatoes. Each of those is a number and a story
about the number, and the story can be wrong even when the number is
right.

This guide is about two questions to ask of any number before you act on
it:

1. **Is the difference real, or is it noise?** Every count and
   measurement wobbles from one time to the next. A difference smaller
   than the usual wobble tells you nothing.
2. **Is the claim honest?** A true number can still be presented to
   mislead: the best result shown as the typical one, a chart with a
   chopped-off axis, a percentage with no "out of how many".

The methods come from the National Institute of Standards and Technology
(NIST), the Centers for Disease Control and Prevention (CDC), the Census
Bureau, the Federal Trade Commission (FTC) and the Securities and
Exchange Commission (SEC). All are United States government
publications. Two were written with others: NIST's statistics handbook
is a joint work with SEMATECH, and the Census Bureau's handbook was drafted with a
private nonprofit, so the guide restates the Census handbook rather than
quoting it. Where something is plain arithmetic,
general practice or our own example, the text says so. The worked
examples use made-up numbers unless a source is named.

Three guides sit beside this one. [Keeping Records](keeping_records.md)
is how you collect numbers worth reading. [Estimating](estimating.md)
covers the two kinds of error, scatter and lean, from the measuring side.
[Units and Converting Them](units_and_converting_them.md) covers the
mistakes that turn a right number into a wrong one.

## First: where a number can hurt you

Most misread numbers cost a bad purchase or a wasted season. A few can
hurt you or someone else, and there the answer is to ask, not to work it
out alone.

- **Medicine and health.** A headline that a treatment "halves the risk"
  or a food "raises the risk" does not tell you what to do. Take the
  question to a doctor or pharmacist, and ask for the numbers both ways:
  how many people in a hundred, or a thousand, with it and without it
  (general practice; the reason is explained under ratios, below).
- **Money.** This guide is about reading claims, not about choosing
  investments, and nothing in it is financial advice. The SEC's
  investor education site lists warning signs of investment fraud,
  including returns that are high with little or no risk and returns
  that are overly consistent. Both are covered below.
- **Safety ratings.** A rated load on a ladder, a jack or a rope is not
  an average you can argue with: stay under it. [Estimating](estimating.md)
  explains why ratings are not for guessing.
- **Test results.** A lab result that says "not detected" does not mean
  zero. It means below the lowest amount the test can tell apart from a
  blank sample, its detection limit. [Testing Water](testing_water.md)
  explains this from the federal definition of a detection limit, and
  how to read a result against it.

## Every measurement is signal plus noise

NIST's handbook of statistical methods describes the general model of
a measurement as a fixed part plus a random part:

    response = deterministic component + random component

In the simplest case, one steady quantity measured again and again, it
says this becomes a constant plus error. In plain words: what you
measure is the thing you care about, plus scatter. The eggs collected on a day depend on how many hens are laying,
which is the signal, and on a dozen small things (heat, a hen that laid
under the hedge, a day you collected late), which are the noise.

NIST also states the assumptions that let you treat a set of numbers as
one steady thing: that they behave like random drawings, from a fixed
distribution, with a fixed location (a steady middle) and a fixed
variation (a steady amount of scatter). Most of the work of reading
numbers honestly is checking whether those hold. A middle that moves is
a change worth knowing about. A scatter that suddenly widens is too.

Measurement error comes in the two kinds [Estimating](estimating.md)
describes from NIST's Technical Note 1297: **random error**, which
scatters results both ways and averages down as you take more readings,
and **systematic error**, a lean in one direction that more readings do
not remove. (That averaging shrinks one and not the other is our reading
of NIST's definitions, as it is in Estimating.) A kitchen scale that
reads 50 g heavy gives a precise, repeatable, wrong number every time.

## Look at the numbers before you summarise them

NIST's chapter on exploratory data analysis describes it as an approach
that relies mostly on graphs, because they let the data show its own
structure before you decide what kind of model it follows. For most
household numbers we suggest starting with the simplest graph, which
NIST calls a **run-sequence plot**: the values in the order they
happened.
NIST says that on such a plot, shifts in the middle and in the scatter
are typically quite evident, and outliers are easy to spot.

You can do this with a pencil. Write the numbers down in time order, or
mark them as dots on squared paper, and look before you average. A
column of weekly egg counts can show:

- a **steady wobble** around one level: one thing, plus noise;
- a **step**: the level jumped and stayed, at a point you can name;
- a **drift**: a slow climb or slide, such as laying that falls as the
  days shorten;
- a **lone odd value**: one week far from the rest.

Each of those means something different, and an average of the whole
column hides all four (our list, illustrating NIST's point).

## Average, middle, most common

NIST defines three ways to say what is typical:

- **Mean**: add the values and divide by how many there are. NIST notes
  this is what people usually mean by "average".
- **Median**: the middle value, with half the data below it and half
  above.
- **Mode**: the value that turns up most often.

They agree when the data are spread evenly on both sides of the middle.
They disagree when the data are lopsided, and NIST explains which way:
the mean is pulled toward the longer tail, and extreme values distort the
mean but not the median. For data with extreme values in the tails, it
says, the median gives a better idea of the typical value.

**Worked example.** Five jars of honey sell at a market for $8, $9, $9,
$10 and $60 (one buyer took a big jar for a wedding). The mean is
$96 / 5 = $19.20. The median is $9. If someone tells you "the average
jar sold for over $19", it is true, and it would badly mislead anyone
pricing their own jars. (Arithmetic on made-up numbers.)

So when you hear "average", ask which one, and whether a few big values
are pulling it.

## Odd values: check them before you throw them out

NIST defines an outlier as an observation that appears to deviate
markedly from the others, and says it may mean bad data, such as a value
written down wrongly, or it may be real random variation or something
interesting. Its advice:

- If you can show the odd value is an error, correct it if you can, or
  leave it out.
- If you cannot tell, do not simply delete it.

At home: an egg count of 120 from twelve hens is almost certainly 12
with a slip of the pen, and your record should say so (see [Keeping
Records](keeping_records.md) on how to correct an entry). A single bad
week after a fox visit is real, and dropping it would make the flock
look healthier than it is.

## How much do the numbers usually wobble?

To know whether a change is real, you need to know how much the number
moves when nothing has changed. NIST lists several measures of this
spread, among them:

- **Range**: the largest value minus the smallest. NIST notes it depends
  only on the two most extreme values and says nothing about the spread
  near the middle.
- **Standard deviation**: roughly, the typical distance of a value from
  the mean, in the same units as the data. It is the square root of the
  variance, which NIST notes can be greatly affected by the values in the
  tails.

For most household decisions the range of a few past seasons, or a few
past weeks, is enough to ask the right question: **is this difference
bigger than the usual swing?** If your bed has given between 30 and 38
kg of potatoes in five normal years, a 36 kg year is not news, and a 22
kg year is (our example).

## Is the difference bigger than the noise?

The Census Bureau publishes almost every figure from its American
Community Survey with a **margin of error**, and its handbook for data
users (September 2020) is one of the clearest public explanations of how
to read one. Restated:

- Any estimate made from a sample differs from the value you would get
  by counting everyone. That difference is **sampling error**.
- The published margins of error are at a **90 percent** confidence
  level. Adding the margin to the estimate and taking it away gives a
  range expected to contain the true value 90 percent of the time.
- The handbook's example: 564,757 one-person households in Colorado in
  2015, with a margin of error of 10,127, gives a range from 554,630 to
  574,884.
- A larger sample gives a smaller standard error, which is why the
  survey combines several years of data for small places.

Comparing two estimates needs a test, not a glance. The handbook's steps,
in short: turn each margin of error into a standard error by dividing it
by 1.645; square both and add them; take the square root; divide the
difference between the two estimates by that. If the result is bigger
than 1.645, the difference is statistically significant at 90 percent.
It also warns not to use overlapping confidence intervals as the test,
because that does not always give the right answer.

The handbook's own example, restated: the share of householders aged 65
or older who live alone was 12.6 percent in Florida (margin 0.2) and
10.5 percent in Arizona (margin 0.3). The test value is about 9.6, far
above 1.645: a real difference. Indiana's 10.8 percent (margin 0.2)
against Arizona's 10.5 gives about 1.37, below 1.645: the handbook says
the user cannot be sufficiently certain the difference is not due to
chance.

The lesson carries to every small comparison you make. **A difference
needs to be judged against how much each number could have wobbled.**
One season of one bed is one roll of the dice. Two beds, two seasons,
and a clear difference both times is evidence (our reading of the
handbook's point).

## You need a comparison group

The CDC's course in epidemiology says the key feature of studies that
look for causes is **a comparison group**. Its example is an outbreak of
hepatitis A traced to a restaurant: asking the sick people which foods
they had eaten only showed which foods were popular. Only when
investigators also asked people who had eaten there and not got sick did
a difference stand out, and it led to the green onions in the salsa.

The same course makes three points worth knowing:

- In an **experiment**, the investigator decides who is exposed, for
  example by random assignment, and follows both groups.
- In an **observational study**, people are simply observed as they are.
  When people with a particular characteristic are more likely than
  those without it to get a disease, the characteristic is said to be
  **associated** with the disease. (Our summary of what follows: an
  association points toward a cause, but does not prove one.)
- In the course's words, "It has been said that epidemiology by itself
  can never prove that a particular exposure caused a particular
  outcome." Often, it adds, it gives enough evidence to act.

At home this is the difference between "I used the new compost and got
a great crop" and knowing the compost did it. The great crop may have
been the weather. Without a bed that got no compost in the same season,
you cannot tell (our application).

## Ratios, rates and "times more likely"

The CDC course describes ratios, proportions and rates as having the
same form: a **numerator over a denominator**. A number with no
denominator cannot be compared with anything. "Forty people got sick at
the fair" means one thing if forty people went, and another if forty
thousand did (our example).

The CDC defines the **risk ratio**, also called relative risk, as the
risk in one group divided by the risk in another. A risk ratio of 1.0
means the same risk in both groups, above 1.0 a higher risk in the first
group, and below 1.0 a lower one.

A ratio does not tell you how big the risk is in the first place, and
that is where honest numbers and misleading ones part. **Relative and
absolute** describe the same change two ways:

- A risk that goes from 2 in 1,000 to 4 in 1,000 has **doubled**: a risk
  ratio of 2.0, "100 percent higher".
- The same change is **2 more people in 1,000**.

Both are true. The first sounds alarming and the second sounds small,
and you need both to decide anything. The same works in reverse for a
product that "halves" a risk. When you only hear the relative number,
ask for the absolute one (arithmetic and general practice).

**Worked example.** A new fence "cuts losses to predators by half". Last
year you lost 2 hens out of 20; this year, with the fence, 1. That is
half, and it is also one hen, which a single fox visit either way could
have produced. Whether the fence is worth it depends on its cost and on
more than one year of records. (Made-up numbers.)

**Percent and percentage points** are another pair that get mixed up.
If seed germination falls from 90 percent to 80 percent, it has fallen
by 10 **percentage points**, which is about 11 **percent** of what it
was. The Census Bureau's handbook uses points the careful way, giving the
width of a range from 23.5 to 36.9 percent as 13 percentage points. A
claim that says "percent" when it means "points", or the other way
round, can make a change look half or twice its size (arithmetic).

## Money across the years

A price or a wage from ten years ago is not in the same units as one
from today, because the dollar has changed. The Census Bureau's handbook
says inflation affects how comparable dollar figures are across time,
that its own multi-year figures are adjusted using the Consumer Price
Index, and that users comparing dollar figures across periods should
adjust for inflation.

**Worked example.** Your income went from $40,000 to $46,000, a rise of
15 percent. If prices rose 20 percent over the same years, $46,000 buys
what about $38,300 used to (46,000 / 1.20). In real terms your income
fell. (Arithmetic on made-up numbers.)

## Charts that mislead

A chart is a number made visible, and it can be made to look like
anything. The Census Bureau's Data Visualization Standards give the
rules that keep a chart honest:

- **Bar charts start at zero.** Its explanation: bar charts use volume to
  show differences, and when they do not start at zero, users risk
  misjudging the difference between values. For data from 0 to 100
  percent, it says to start at 0 percent.
- **Charts meant to be compared use the same axes.** It shows two charts
  whose bars look the same height but differ by thirty percentage
  points, because their scales differ.
- **No 3D bars**, which it says distort the eye's judgement of volume.

**Worked example.** Two yields of 92 and 94 drawn as bars on an axis
that starts at 90 come out 2 and 4 units tall. The second bar looks
twice as big for a difference of about 2 percent (arithmetic).

When a chart surprises you, read the numbers on its axis before you
believe its shape.

## The best result, shown as the typical one

A seller showing you its happiest customers is showing you the top of
the range, not the middle. The FTC's Guides on endorsements and
testimonials (16 CFR 255.2) say that an advertisement in which customers
describe their results on a key feature will likely be read as saying
those results are what customers will generally get, and that the
advertiser must be able to back that up. They also say: "Consumer
endorsements themselves are not competent and reliable scientific
evidence."

The FTC's own example is homestead-relevant. A heat pump advertisement
shows three customers whose monthly bills fell by $100, $125 and $150,
when in fact fewer than 20 percent of buyers save $100 or more. The FTC
says a line like "Results not typical" does not fix that, because people
still take the examples as typical. What the advertiser should do is
disclose the generally expected savings, such as what the average
homeowner saves, and be able to back that up. The FTC adds a further
catch: a figure based on customers in a warm climate can still mislead
people in a cold one.

The same section says sellers should not suppress, boost or edit reviews
in ways that distort what customers think.

So, for any claim built on stories: **ask what most buyers get, and
where they live.**

## Too smooth to be true

Real numbers wobble. That is the noise this guide began with, and its
absence is a warning sign of its own. The SEC lists overly consistent
returns among the red flags of an investment fraud known as a Ponzi
scheme: investments tend to go up and down over time, it says, so be
skeptical of one that regularly makes positive returns regardless of
overall market conditions. It also lists high returns with little or no
risk, and tells readers to be highly suspicious of any "guaranteed"
investment opportunity.

The same reasoning applies beyond money. A seller whose records show
exactly the same yield every year, or a test that has never once come
back different, may be telling you about the record-keeping rather than
the crop (our extension of the SEC's point).

## A checklist for any number

General practice, drawn from the sections above:

1. **Compared to what?** Is there a comparison group, a baseline, or
   last year's figure?
2. **Out of how many?** What is the denominator?
3. **How much does it usually wobble?** Is the difference bigger than
   that?
4. **Which average?** Mean or median, and are a few big values pulling
   it?
5. **Relative or absolute?** If you hear "doubles" or "halves", what
   are the numbers before and after?
6. **Percent or points?**
7. **Which year's dollars?**
8. **Does the chart start at zero, and do compared charts share an
   axis?**
9. **Typical or best?** Is this what most people get?
10. **Who is telling you, and what do they gain?**

## Worked example: did the new compost help?

An illustration with made-up numbers, putting the pieces together.

Last year you added a new compost to your potato bed and harvested 41
kg, against 36 kg the year before. Is the compost working?

1. **Look at the record.** Your log shows the bed gave 33, 38, 36, 30
   and 36 kg in the five years before. The range is 30 to 38 kg, a swing
   of 8 kg in normal years. A 41 kg year is a little above the old range,
   but by less than the usual swing. Promising, not proven.
2. **Ask about the weather.** Your rain record shows last summer was
   wetter than usual. A good year for every bed would also explain it.
3. **Find a comparison.** The neighbour's bed, without the compost, also
   did well. That points at the weather.
4. **Design a fair test.** This year, split the bed in two. Toss a coin
   to decide which half gets the compost. Same seed potatoes, same
   planting day, same watering. Next year, swap the halves. The coin
   stops you giving the compost the better end of the bed without
   meaning to; the swap stops one end of the bed being better anyway.
   (This is the experiment the CDC describes, scaled down to a garden;
   the coin and the swap are general practice.)
5. **Decide in advance what would convince you.** For example: the
   compost half wins in both years, by more than the two halves differ
   in a year when neither gets compost. If you have no such year, grow
   one first: a season with both halves untreated shows how much the two
   ends of the bed differ on their own. The bed's 8 kg swing between
   years is only a rough yardstick here, because it includes the
   weather, which a same-season split cancels out. Writing the rule down
   before the harvest stops you moving the goalposts afterwards (general
   practice).

## Know where your own reading stops

- **Your health.** Read the numbers, then decide with a doctor or
  pharmacist. Ask for absolute numbers.
- **Your money.** Use the SEC's red flags, check that a seller and an
  investment are registered, and get advice from someone qualified who
  is not selling you the product (the registration check is the SEC's
  advice; the rest is general practice).
- **Formal studies and surveys.** Working out how big a sample you need,
  or analysing a study to publish or to base a public decision on, is
  work for someone trained in statistics (general practice).
- **Lab tests.** The lab, and the agency that sets the standard, decide
  what a result means. See [Testing Water](testing_water.md).

## How the game models it

The game is a good place to watch noise, because some of its numbers are
random on purpose and others are smoothed.

- **Harvests are rolled.** When you harvest, the game picks the crop's
  yield at random between its low and high figure for each plant
  (`yield_min` and `yield_max` in `data/plants.csv`), scaled up by the
  number of plants in the bed, then scales it by how well the crop did:
  its health over the season and, for crops that need pollinating, how
  well it set fruit. A fractional result is rounded up or down at
  random, in proportion, so that the average comes out right over many
  harvests. Two beds planted and tended the same way can therefore give
  different harvests. Before deciding that one bed or one way of tending
  is better, compare several harvests, not one.
- **The same machine, three ways.** Press F2 for the performance
  overlay. Its FPS and frame-time figures are for the single most recent
  frame, so they jump about. Below them, a small bar graph shows the last
  120 frames, each bar marked by whether that frame met 60 frames a
  second, 30, or neither: a run-sequence plot. The Performance page
  (under Platform) shows smoothed figures instead, where each new frame
  counts for 15 percent of the number shown. The raw number is noisy,
  the smoothed number hides a single slow frame, and only the plot shows
  both.
- **Averages you can check.** The Construction page counts each machine
  at its average power draw over a day; [Estimating](estimating.md)
  shows how to check it by hand.

**One setting changes this.** Settings > Gameplay > "Start every session
from the default home" is off by default, so your garden carries over
between launches. With the setting on, only your character carries
between launches, and the garden starts over at each launch. Either way
the game keeps no tally of your harvests, so keep one on paper rather
than relying on the game to remember it.

What the game does not model: the prices at the trading post are fixed,
so there is no market noise to read there. And the Trading skill, which
this topic is meant to train, exists in the skill list, but nothing in
the game levels it yet.

## You own this when

- You write numbers down in time order and look at them before you
  average them.
- You say which average you mean, and use the median when a few big
  values would pull the mean.
- You check an odd value against the record before you keep it or drop
  it.
- You know how much a number you care about usually wobbles, and you do
  not act on a difference smaller than that.
- You can turn a margin of error into a range, and you know why
  overlapping ranges are not a test.
- You ask for a comparison group before you believe that something
  caused something.
- You ask for the denominator, the absolute numbers behind a ratio, and
  whether it is percent or points.
- You adjust money for inflation before comparing years.
- You read a chart's axis before its shape.
- You ask what most buyers get, not what the best ones got, and you are
  wary of numbers that never wobble.
- You can design a fair trial in your own garden, and decide in advance
  what result would convince you.

## Sources

Grouped by what kind of authority each one is. Every source here is a
United States federal government publication. Two were written with
others: the NIST/SEMATECH e-Handbook is a joint work, and only its
chapter 1, edited at NIST, is cited; the Census Bureau's handbook was
drafted with the Population Reference Bureau, a private nonprofit, so it
is restated, not quoted. The rest are works of the federal government
and in the public domain. Regulations were read in the Electronic Code of Federal
Regulations on 3 October 2026; the eCFR is updated in place. Web pages
were read on 3 October 2026.

### United States government (public domain)

- National Institute of Standards and Technology. *NIST/SEMATECH
  e-Handbook of Statistical Methods*, chapter 1, Exploratory Data
  Analysis (chapter editor James J. Filliben, NIST), https://doi.org/10.18434/M32189,
  created 1 June 2003, last updated 27 April 2022. Sections read: 1.1.1
  What is EDA? (mostly graphical, letting the data reveal its structure);
  1.2.1 Underlying Assumptions (random drawings from a fixed distribution
  with fixed location and variation; the general model, response =
  deterministic component + random component, becoming constant + error
  in the univariate case); 1.3.3.25 Run-Sequence Plot (shifts in location and
  scale typically evident, outliers easy to see); 1.3.5.1 Measures of
  Location (mean, median and mode; the mean pulled toward the longer
  tail; extreme values distorting the mean but not the median); 1.3.5.6
  Measures of Scale (range based only on the extremes; standard
  deviation in the data's units and affected by the tails); 1.3.5.17
  Detection of Outliers (an outlier may be bad data or real variation;
  correct or delete proven errors, do not simply delete the rest).
  https://www.itl.nist.gov/div898/handbook/eda/section3/eda351.htm and
  neighbouring pages.
- National Institute of Standards and Technology. Taylor, B. N. and
  Kuyatt, C. E. *Guidelines for Evaluating and Expressing the
  Uncertainty of NIST Measurement Results*, NIST Technical Note 1297,
  1994 edition, appendix D (random and systematic error, as described in
  [Estimating](estimating.md); restated, not quoted, because the
  definitions reproduce the international vocabulary of metrology).
  https://nvlpubs.nist.gov/nistpubs/Legacy/TN/nbstechnicalnote1297.pdf
- Centers for Disease Control and Prevention. *Principles of
  Epidemiology in Public Health Practice*, Third Edition. Lesson 1,
  section 7, Analytic Epidemiology, page last reviewed 18 May 2012 (the
  comparison group as the key feature; the hepatitis A and salsa
  investigation; experimental and observational studies; association,
  as people with a characteristic being more likely than those without
  it to get a disease;
  "It has been said that epidemiology by itself can never prove that a
  particular exposure caused a particular outcome").
  https://archive.cdc.gov/www_cdc_gov/csels/dsepd/ss1978/lesson1/section7.html
  Lesson 3, sections 1 and 5 (ratios, proportions and rates as a
  numerator over a denominator; the risk ratio or relative risk and
  what values above and below 1.0 mean).
  https://archive.cdc.gov/www_cdc_gov/csels/dsepd/ss1978/lesson3/section5.html
- US Census Bureau. *Understanding and Using American Community Survey
  Data: What All Data Users Need to Know*, September 2020, drafted with
  the Population Reference Bureau; restated, not quoted (sampling and
  nonsampling error; margins of error at 90 percent; the Colorado
  confidence interval; standard error as the margin divided by 1.645;
  larger samples giving smaller standard errors; the steps of the
  significance test and the Florida, Arizona and Indiana example; not
  relying on overlapping confidence intervals; a range from 23.5 to 36.9
  percent described as 13 percentage points; adjusting dollar figures
  for inflation with the Consumer Price Index).
  https://www.census.gov/content/dam/Census/library/publications/2020/acs/acs_general_handbook_2020.pdf
- US Census Bureau. Data Visualization Standards (beta), undated (bar
  charts should always start at zero, because they use volume to show
  differences; tick marks starting at 0 percent for data from 0 to 100
  percent; consistent axes for charts meant to be compared; no 3D
  graphics). https://xdgov.github.io/data-design-standards/visualizations/bar-chart
  and https://xdgov.github.io/data-design-standards/components/axes
- Federal Trade Commission. 16 CFR 255.2, Consumer endorsements, in the
  Guides Concerning the Use of Endorsements and Testimonials in
  Advertising (testimonials on a key attribute read as representative of
  what consumers generally achieve; "Consumer endorsements themselves are
  not competent and reliable scientific evidence"; the heat pump example
  with savings of $100, $125 and $150 and fewer than 20 percent of buyers
  saving $100 or more; disclaimers such as "Results not typical" not
  curing it; disclosing generally expected results, and the climate
  caveat; not distorting consumer reviews).
  https://www.ecfr.gov/current/title-16/chapter-I/subchapter-B/part-255/section-255.2
- US Securities and Exchange Commission, Office of Investor Education
  and Advocacy. Ponzi Scheme, on Investor.gov, undated (red flags,
  including high returns with little or no risk, "guaranteed"
  opportunities, overly consistent returns regardless of market
  conditions, and unregistered investments and sellers).
  https://www.investor.gov/protect-your-investments/fraud/types-fraud/ponzi-scheme

### Inside this project

- The harvest roll: `harvest_quantity` in `src/systems/farming/mod.rs`
  and its caller in `src/systems/farming/picking.rs`; the yields in
  `data/plants.csv`, with their sources in `data/garden/yields.ron`.
- The F2 performance overlay and its 120-frame bar graph:
  `src/gui/pages/diagnostics.rs`, with the per-frame figure set in
  `src/lib.rs`; the Performance page and its smoothing
  (`EMA_ALPHA` in `src/renderer/frame_costs.rs`,
  `src/gui/pages/performance.rs`).
- The "Start every session from the default home" setting
  (`src/config.rs`, `src/save_load.rs`); the Trading skill in
  `data/skills/skills.csv`, which nothing awards experience to.
- [Keeping Records](keeping_records.md), [Estimating](estimating.md),
  [Units and Converting Them](units_and_converting_them.md) and [Testing
  Water](testing_water.md).

### Labelled in the text as arithmetic, general practice or our example, not sourced

- The worked examples (honey prices, potato beds, the fence and the
  hens, germination, income and inflation, the bar chart, the compost
  trial) use made-up numbers and plain arithmetic.
- The four patterns in a column of numbers (wobble, step, drift, odd
  value) illustrate NIST's point; the egg and fox examples are ours.
- Asking for absolute numbers, taking health questions to a doctor or
  pharmacist, and getting money advice from someone who is not selling
  the product.
- The meaning we draw for a garden from the Census Bureau's test and the
  CDC's comparison group; the coin toss and the swap in the fair trial;
  deciding in advance what would convince you, and using an untreated
  year of both halves as the yardstick.
- That an association points toward a cause but does not prove one is
  our summary of the CDC's points, not its words.
- Extending the SEC's warning about overly consistent returns to records
  and tests outside investing.
- That averaging shrinks random error but not systematic error is our
  reading of NIST's definitions in Technical Note 1297.
- Leaving formal sample-size and study design to someone trained in
  statistics.
