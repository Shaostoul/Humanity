# Choosing a Staff Tube: Alloys, Walls and Length

Every flow-arts staff, whether it carries fire wicks, LED heads or plain
practice ends, starts as a length of tube. This guide covers the three
choices that decide what that tube will be like to spin: how long it is,
how thick it is, and what metal it is made of. It also covers the question
that follows a metal tube into a fire: which metals, platings and coatings
give off harmful fumes when heated, and which are only a hazard while you
are cutting, grinding or welding them.

It does not cover wicks, grips, heat shields or the hardware that holds a
wick on. Those are in the fire staff guide,
[Building a Fire Staff](/library#fire-staff-materials). Fuels are in
[Fire Performance Fuels](/library#fire-performance-fuels). For why metals
bend, dent and corrode the way they do, see
[Metals and Alloys](/library#metals-and-alloys).

One hard boundary before anything else: **learn fire spinning in person,
from experienced spinners, with a trained spotter holding a fire blanket,
and never from a document.** Learn the movement on an unlit staff first.
This guide can help you choose a tube that will not bend, dent, overheat or
poison you. It cannot make you safe with fire.

Two honest limits run through everything below.

First, **nobody appears to have published a measurement of how hot the
metal under a burning wick actually gets.** Every heat judgement in this
guide compares a material's published rating against an assumed range of
roughly 200 to 400 C for the metal near the wick, with direct flame contact
possibly hotter. Where that assumption carries a conclusion, the text says
so.

Second, many of the numbers here (weights, stiffness, bending strength,
dent resistance, heat carried) are **computed from published material
properties**, not measured on real staffs. The inputs are specification
minimums and manufacturer data, cited at the bottom. The calculation is
reproducible: the tables were computed by `tables.js` from
`alloys-final.json`, both kept with the
[research notes](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/research/2026-09-24-fire-performance/README.md)
in the project's source repository (they are not shipped inside the app),
so they can be regenerated from the data whenever a property is
corrected.

Where a figure comes only from staff makers, retailers or practitioners,
it is labelled **community figure**. Prices and stock were checked on
**September 24, 2026** and are a snapshot that will age quickly.

## Quick terms

- **OD, ID and wall.** Outside diameter, inside diameter, and the
  thickness of the metal. ID is OD minus two walls.
- **Stiffness and strength are different things.** Stiffness is how much
  the staff flexes under a load, the bounce you feel. Strength is how much
  load it takes before it bends or dents permanently. A staff can be stiff
  and weak, or flexible and strong.
- **Yield strength** is the stress at which metal stops springing back.
  Given in MPa or ksi (1 ksi is about 6.9 MPa).
- **Specification minimum and typical value.** A minimum is the lowest
  strength a standard allows the mill to ship. A typical value is what an
  average piece measures, and reads higher. This guide uses minimums
  wherever one exists.
- **Temper.** The heat treatment or cold work that gives an alloy its
  strength. T6 on aluminium means heat treated and artificially aged.
  Annealed means softened. CWSR, on titanium, means cold worked and stress
  relieved.
- **Titanium grades.** Grades 1 to 4 are commercially pure (CP) titanium,
  softest to strongest. Grade 5 is Ti-6Al-4V. Grade 9 is Ti-3Al-2.5V.
  Grade 23 is a low-oxygen grade 5 made for implants. Grade 12 is a
  corrosion grade with a little molybdenum and nickel.

## Length

Decide length first, because it changes what diameter and wall feel right.

### What is on sale, and the common rules

Contact staffs on sale in September 2026 mostly came in 54 to 60 inches,
with 59 to 60 inches (150 to 152 cm) the most common and longer lengths
made to order (**community figure**, from maker and retailer listings).

The sizing rules are makers' and teachers' rules of thumb, not studies:

- **Floor to chin** is the most common rule for a contact staff. Some put
  the mark at the bottom of the chin, some between the lower lip and the
  chin.
- Other makers give a wider window: chest to chin, shoulder to nose, chin
  to eyes, or your height minus about 15 cm. One well-known retailer uses
  the top of the shoulder, lower than most.
- A "height minus about 20 cm (8 inches)" rule lands close to chin height
  for someone of average height, and drifts for people who are very tall
  or very short.

Treat it as a range from roughly chest or shoulder height up to eye level,
with the chin as the usual starting point. All of this is **community
figure**.

### How to measure yourself, step by step

1. Stand straight against a wall, in the shoes you practise in. (No source
   says barefoot or shod; practising shoes simply match how you will
   stand.)
2. Put the tongue of a tape measure under your heel.
3. Have a helper hold a book flat and level against the wall, touching the
   underside of your chin. Measure from the floor to the underside of the
   book. That is your starting contact staff length.
4. Sanity check without a tape: chin height is roughly 0.87 of standing
   height. That ratio comes from body-proportion tables published in 1966
   that later researchers describe as ambiguously defined and never
   validated, so use it only as a check. By it, a 60 inch staff is chin
   height for someone about 5 ft 9 in tall.
5. Round to the nearest length you can buy or cut.
6. Adjust for style. Go a few inches shorter (makers suggest up to about
   8 inches under the chin) for fast, dancey flow or for mixing in spin
   moves. Go longer for long, slow rolls, but not past eye level, where
   some horizontal moves become very hard.
7. For a fire contact staff, makers size the overall length, wicks
   included, the same way as a practice staff. The one published allowance
   for wicks (add 4 to 6 inches) is for dragon staffs with end wicks, not
   contact staffs.
8. **Test before you cut good tube.** A broomstick or dowel, shortened a
   little at a time, answers the question for almost nothing. You can
   shorten a staff; you can never lengthen it.
9. Check that the ends clear the floor, your face and the ceiling in the
   moves you actually do.
10. Do not agonise over an inch or two. Every source agrees personal
    preference wins in the end.

### Why a few inches matter more than they look

Makers agree on the trade: a longer staff turns more slowly, carries more
momentum and gives more leverage, which suits slow flowing rolls. A
shorter one starts and stops faster, leaves more room around the body, and
is easier to throw.

The physics explains the size of the effect. A plain tube's resistance to
being turned about its centre grows with its mass times its length
squared (the textbook result for a rod, as HyperPhysics gives it), and the mass itself grows with length, so for the same tube it
grows with the **cube** of the length. A 60 inch tube has about 37 percent
more resistance to turning than a 54 inch tube of the same section. End
weights and wicks change the exact figure, but not the direction.

Longer also weighs more, is harder to travel with unless it breaks down,
hits the ground more often, and is easier to bruise yourself with.

### Children and smaller adults

There is no separate children's rule, because the body-landmark rules
scale with the person. One maker, Phoenix Fire Props, publishes the only
chart found that reaches children's heights (**community figure**):

| Your height | Staff length |
|---|---|
| 3 ft 6 in to 3 ft 11 in | 3 ft |
| 4 ft 0 in to 4 ft 4 in | 3.5 ft |
| 4 ft 5 in to 4 ft 11 in | 4 ft |
| 5 ft 0 in to 5 ft 7 in | 4.5 ft |
| 5 ft 8 in to 6 ft 3 in | 5 ft |
| 6 ft 4 in to 6 ft 10 in | 5.5 ft |
| 6 ft 11 in to 7 ft 4 in | 6 ft |

Each band covers 5 to 8 inches of height, so someone at the tall end of a
band gets a staff below chin height. For thinner hands, one contact staff
maker, RandyLeeSticks, usually recommends 5/8 inch tube for people under
about 5 ft 4 in (**community figure**). No source gives a minimum age. Children learn on
practice staffs; remember too that a shorter staff puts any flame closer
to the body.

### Ceilings

No flow-arts source gives a ceiling height for practice. As rough
arithmetic from general body-measurement tables: holding a staff by its
centre at full overhead reach puts the tip at about 2.86 m (9.4 ft) for an
average man with a 60 inch staff, and about 2.64 m (8.7 ft) for an average
woman with a 55 inch staff. The US residential building code allows
habitable rooms with ceilings as low as 7 ft, so overhead vertical moves
indoors often need a shorter staff or a taller room. Lights and ceiling
fans lower the real clearance.

Fire indoors is a separate matter and is not a length question. Portland
Fire & Rescue's fire performance policy, as one example, requires an indoor
fire performance venue to have an automatic sprinkler system and a ceiling
of at least 12 ft.

### Other staff types, for comparison

All **community figure**.

| Staff | Typical length | Typical tube |
|---|---|---|
| Contact staff | Floor to chin, 54 to 60 in common | 3/4 in (19 mm); 5/8 and 7/8 in also common |
| Spin (manipulation) staff | Armpit or chest up to chin; 120 to 130 cm is a typical first staff | 3/4 or 7/8 in |
| Double staffs | 26 to 36 in, 32 in most common; held at its centre, the end should tuck between your arm and torso, around mid upper arm | Usually 1/2 in (13 mm) |
| Dragon staff | Varies most: chin to nose or eye level (Sacred Flow Art, by wick count), or the top of the head (Home of Poi) | Mostly 7/8 in (22 mm) |

Many spinners will not go below 32 inches for fire double staffs because
of the heat. That is a double-staff rule; it is sometimes misquoted as a
contact-staff rule.

## Diameter and wall thickness

### Diameter

Contact staffs are built mostly on 3/4 inch (19 mm), 5/8 inch (16 mm) or
7/8 inch (22 mm) tube. In the US, 3/4 inch is often called the standard
and 5/8 inch is suggested for smaller hands. Outside the US, metric 20 to
22 mm tube turns up too. The Australian maker Threeworlds uses 20 mm on one
staff and 22 mm (7/8 inch) on its Fusion system, which US shops such as The
Spinsterz resell. At least one US maker, Renegade Juggling, also uses 20 mm
(**community figure**).

Makers describe the feel differently. A thinner tube rolls across the body
more slowly; some say that gives more control for traps and wrist moves,
others that it demands more control and is less grippy. A fatter tube is
easier to grab, rolls faster and flexes less. 3/4 inch is the middle
ground. Grip layers add to the diameter you actually hold.

**Diameter changes stiffness far more than wall does.** Computed from tube
geometry: going from 3/4 to 7/8 inch at the same 0.058 inch wall makes a
tube about 64 percent stiffer, while going from a 0.049 to a 0.065 inch
wall on a 3/4 inch tube adds only about 24 percent. If a staff is too
bouncy, a slightly larger diameter fixes it more efficiently than a
thicker wall.

### What walls makers use

Most makers do not publish wall thickness at all. Those that do
(**community figure**):

- 7075-T6 contact staff tube at 1.4 mm (0.055 in) wall is common: Dark
  Monk at 19 x 1.4 mm, Ninja Pyrate at 3/4 x 0.056 in, Buy Fire Fans at
  19.1 x 1.4 mm. Dark Monk does not recommend its 1.1 mm walls for contact
  staffs.
- One Australian maker's 20 mm staff uses 1.6 mm 6063 in its specification
  section, while its marketing copy on the same page says 7075. Trust the
  specification section, or ask.
- Carbon fibre staff tube runs about 1.5 to 2 mm wall (2 mm for Dark
  Monk's carbon fibre tube, 1.5 mm for the Flowbonacci dragon staff).
- One maker's rule for hardware-store aluminium is a wall of at least
  1.6 mm (0.06 in), or it bends too easily. That rule is for soft tube,
  usually 6063. Strong 7075 gets away with 1.4 mm.

So for a 3/4 inch contact staff, aluminium walls of about 0.055 to 0.065
inch (1.4 to 1.6 mm) are the norm. For soft tube, the 1.6 mm rule only
reduces the problem; it does not fix it. Computed the same way as the
tables below, a 3/4 x 1/16 inch hardware tube in 6063-T5 flexes about like
a standard staff but keeps only about 0.44 of its bending strength, a
little under half, so even at that wall it stays an LED and practice tube.
Unlabelled hardware tube could be that temper or the stronger 6063-T6, and
you cannot tell which by looking.

### What changing the wall does

- Weight, stiffness and bending strength all rise roughly in step with the
  wall.
- **Dent resistance rises faster than the wall.** As an engineering estimate, not a
  measurement, common models for crushing a tube's wall scale it between
  the wall to the power 1.5 and the wall squared. Halve the wall and you keep only about a
  quarter to a third of the dent resistance.
- A dent or ding makes a later bend kink much sooner. That is reasoning
  from how tubes fail, not a test.
- In aluminium, going from a 0.035 to a 0.065 inch wall on a 5 ft tube
  adds about 160 g, spread along the middle (about 270 g in titanium and
  475 g in stainless). In aluminium that is roughly one end weight's worth,
  so a thicker wall also makes a staff feel less end-heavy.
- **Thinner is not always cheaper.** Price follows how common a size is.
  On the snapshot date, 60 inches of 3/4 inch 304 stainless cost $43 at
  0.035 inch wall but $30 at 0.065 inch, and 4130 chromoly cost $9.95 per
  foot at 0.028 inch but $5.35 per foot at 0.065 inch.
- **A thin wall cannot hold threads.** A screw tapped straight through a
  0.040 inch wall catches only one or two threads (about 1.3 for a No. 8-32
  or 10-32 screw), and cutting a thread into the tube itself removes a
  large fraction of the wall right where drops land. The usual answer is a
  close-fitting plug inside the end, with the thread in the plug. The fire
  staff guide covers end hardware.

### The inside diameter decides what fits

- **Wood plugs.** 3/4 inch tube is usually said to take a 5/8 inch
  (0.625 in) dowel (Dark Monk says so of its tube; **community figure**).
  Check the wall: a 0.065 inch wall leaves 0.620 inch
  inside and a 1/16 inch hardware wall exactly 0.625 inch, so a 5/8 dowel
  will be very tight or will not go in without sanding. A 0.049 inch wall
  (0.652 in) takes it easily; a 0.040 inch wall (0.670 in) leaves it loose.
  Dowels vary, so measure both before buying in bulk.
- **LED capsules.** Standard Flowtoys LED capsules are 21 mm across. The
  inside of a 3/4 inch metal tube is only about 15.7 to 17.3 mm, and of a
  7/8 inch tube about 18.9 to 19.7 mm, so they do not fit. LED staffs built
  on those capsules carry them in clear 1 inch polycarbonate end tubes. A
  capsule would fit inside 1 x 0.049 inch metal tube (22.9 mm inside), but
  the metal would block the light.
- **Telescoping.** A 7/8 x 0.058 inch sleeve over 3/4 inch tube leaves
  only about 0.009 inch of nominal clearance, which is less than normal
  tube tolerances, so it may slide, bind or refuse to go on. 7/8 x 0.049
  is loose and 7/8 x 0.065 is nominally too small. Test-fit real pieces.
- **End weights.** A thinner wall leaves more room inside for plugs and
  weights, which matters if you want to move mass to the ends.

### The wall sweep

These tables use the same method and baseline as the alloy comparison
below (the indices are explained there). Each is a bare 60 inch, 3/4 inch
OD tube, compared with a 3/4 x 0.065 inch 6061-T6 tube, which scores 1.00.
Inside diameters are the same for every metal at a given wall.

**Aluminium 6061-T6** (tube minimum)

| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |
|---|---|---|---|---|---|
| 0.028 (0.71) | 0.694 | 169 (0.37) | 0.50 | 0.50 | 0.19 |
| 0.035 (0.89) | 0.680 | 209 (0.46) | 0.61 | 0.61 | 0.29 |
| 0.040 (1.02) | 0.670 | 237 (0.52) | 0.68 | 0.68 | 0.38 |
| 0.049 (1.24) | 0.652 | 286 (0.63) | 0.80 | 0.80 | 0.57 |
| 0.058 (1.47) | 0.634 | 335 (0.74) | 0.92 | 0.92 | 0.80 |
| 0.065 (1.65) | 0.620 | 371 (0.82) | 1.00 | 1.00 | 1.00 |
| 0.083 (2.11) | 0.584 | 462 (1.02) | 1.19 | 1.19 | 1.63 |

**Aluminium 7075-T6** (tube minimum)

| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |
|---|---|---|---|---|---|
| 0.028 (0.71) | 0.694 | 175 (0.39) | 0.52 | 0.95 | 0.35 |
| 0.035 (0.89) | 0.680 | 216 (0.48) | 0.63 | 1.15 | 0.55 |
| 0.040 (1.02) | 0.670 | 246 (0.54) | 0.70 | 1.29 | 0.71 |
| 0.049 (1.24) | 0.652 | 297 (0.65) | 0.83 | 1.52 | 1.07 |
| 0.058 (1.47) | 0.634 | 347 (0.77) | 0.95 | 1.73 | 1.50 |
| 0.065 (1.65) | 0.620 | 385 (0.85) | 1.03 | 1.89 | 1.89 |
| 0.083 (2.11) | 0.584 | 479 (1.06) | 1.22 | 2.24 | 3.08 |

**Titanium grade 2** (tube minimum)

| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |
|---|---|---|---|---|---|
| 0.028 (0.71) | 0.694 | 282 (0.62) | 0.76 | 0.57 | 0.21 |
| 0.035 (0.89) | 0.680 | 349 (0.77) | 0.93 | 0.69 | 0.33 |
| 0.040 (1.02) | 0.670 | 396 (0.87) | 1.04 | 0.78 | 0.43 |
| 0.049 (1.24) | 0.652 | 479 (1.05) | 1.23 | 0.92 | 0.65 |
| 0.058 (1.47) | 0.634 | 559 (1.23) | 1.40 | 1.05 | 0.91 |
| 0.065 (1.65) | 0.620 | 620 (1.37) | 1.52 | 1.14 | 1.14 |
| 0.083 (2.11) | 0.584 | 771 (1.70) | 1.81 | 1.35 | 1.86 |

**Titanium grade 9 CWSR** (tube minimum)

| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |
|---|---|---|---|---|---|
| 0.028 (0.71) | 0.694 | 280 (0.62) | 0.73 | 1.51 | 0.56 |
| 0.035 (0.89) | 0.680 | 346 (0.76) | 0.88 | 1.83 | 0.87 |
| 0.040 (1.02) | 0.670 | 393 (0.87) | 0.99 | 2.05 | 1.14 |
| 0.049 (1.24) | 0.652 | 475 (1.05) | 1.17 | 2.42 | 1.71 |
| 0.058 (1.47) | 0.634 | 555 (1.22) | 1.33 | 2.76 | 2.40 |
| 0.065 (1.65) | 0.620 | 616 (1.36) | 1.45 | 3.01 | 3.01 |
| 0.083 (2.11) | 0.584 | 766 (1.69) | 1.72 | 3.57 | 4.91 |

**Titanium grade 5** (one tube maker's minimum; see the titanium section)

| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |
|---|---|---|---|---|---|
| 0.028 (0.71) | 0.694 | 277 (0.61) | 0.83 | 1.45 | 0.54 |
| 0.035 (0.89) | 0.680 | 342 (0.75) | 1.01 | 1.77 | 0.84 |
| 0.040 (1.02) | 0.670 | 389 (0.86) | 1.13 | 1.98 | 1.10 |
| 0.049 (1.24) | 0.652 | 470 (1.04) | 1.33 | 2.34 | 1.65 |
| 0.058 (1.47) | 0.634 | 549 (1.21) | 1.52 | 2.67 | 2.31 |
| 0.065 (1.65) | 0.620 | 609 (1.34) | 1.65 | 2.90 | 2.90 |
| 0.083 (2.11) | 0.584 | 758 (1.67) | 1.96 | 3.45 | 4.74 |

**Stainless 304, annealed** (tube minimum)

| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |
|---|---|---|---|---|---|
| 0.028 (0.71) | 0.694 | 493 (1.09) | 1.45 | 0.43 | 0.16 |
| 0.035 (0.89) | 0.680 | 611 (1.35) | 1.77 | 0.52 | 0.25 |
| 0.040 (1.02) | 0.670 | 693 (1.53) | 1.98 | 0.59 | 0.33 |
| 0.049 (1.24) | 0.652 | 838 (1.85) | 2.34 | 0.69 | 0.49 |
| 0.058 (1.47) | 0.634 | 979 (2.16) | 2.67 | 0.79 | 0.68 |
| 0.065 (1.65) | 0.620 | 1087 (2.40) | 2.90 | 0.86 | 0.86 |
| 0.083 (2.11) | 0.584 | 1351 (2.98) | 3.44 | 1.02 | 1.40 |

**4130 chromoly, normalized** (tube minimum)

| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |
|---|---|---|---|---|---|
| 0.028 (0.71) | 0.694 | 490 (1.08) | 1.49 | 1.07 | 0.40 |
| 0.035 (0.89) | 0.680 | 607 (1.34) | 1.81 | 1.31 | 0.62 |
| 0.040 (1.02) | 0.670 | 689 (1.52) | 2.03 | 1.46 | 0.81 |
| 0.049 (1.24) | 0.652 | 833 (1.84) | 2.39 | 1.73 | 1.22 |
| 0.058 (1.47) | 0.634 | 973 (2.15) | 2.73 | 1.97 | 1.71 |
| 0.065 (1.65) | 0.620 | 1080 (2.38) | 2.98 | 2.15 | 2.15 |
| 0.083 (2.11) | 0.584 | 1342 (2.96) | 3.53 | 2.55 | 3.50 |

## How to read the alloy tables

Every entry is a bare 60 inch x 3/4 inch OD tube with no plugs, wicks or
grip. The baseline is a standard 3/4 x 0.065 inch 6061-T6 aluminium tube
weighing 371 g, which scores 1.00 on every index. A score of 2.00 means
twice the baseline; 0.50 means half.

- **Stiffness** is resistance to flexing: the elastic modulus times the
  tube's bending stiffness from its shape. At 2.00 the tube flexes half as
  much under the same load.
- **Bending strength** is the bending load the tube takes before it bends
  for good: the alloy's minimum yield strength times the tube's section
  from its shape. At 2.00 it takes twice the load.
- **Dent index** is yield strength times wall thickness squared, a scaling
  used for crushing the wall of a ring. **It is a rough comparison for
  ranking tubes against each other, not a prediction.** It ignores how
  ductile a metal is, how much it hardens as it deforms, and what a real
  impact on a real edge does. Nobody has published a drop test for staff
  tube. At the 0.065 inch wall the dent index equals bending strength for
  every alloy, by how both are defined, so it is not repeated in that
  table.
- **Heat carried** is thermal conductivity times the metal's cross
  section: how readily the tube walls conduct heat from the wick zone
  toward your hands. Lower keeps your hands cooler, but also keeps heat
  concentrated in the metal right under the wick.
- **Strength basis.** "Tube min" is a published specification minimum for
  tube in that alloy and condition. "Sheet min", "strip min", "bar min" and
  "bar/plate min" are minimums for another product form, used because no
  tube minimum was found. "Typical" is an average value, not a guaranteed
  minimum, so those rows read high next to the others.

The property behind each row, and the document it came from, is recorded
in the `note` field of `alloys-final.json` in the research notes (source
repository only), and the full calculator output is in
[tables.md](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/research/2026-09-24-fire-performance/tables.md).

## The alloy comparison

### 60 in x 3/4 in OD x 0.040 in wall, every alloy

| Alloy | Strength basis | Weight g (lb) | Stiffness | Bending strength | Dent index | Heat carried |
|---|---|---|---|---|---|---|
| Aluminium 5052-H32 | tube min | 235 (0.52) | 0.69 | 0.45 | 0.25 | 0.53 |
| Aluminium 6061-T6 | tube min | 237 (0.52) | 0.68 | 0.68 | 0.38 | 0.64 |
| Aluminium 6063-T5 | tube min | 237 (0.52) | 0.68 | 0.31 | 0.17 | 0.80 |
| Aluminium 6063-T6 | tube min | 237 (0.52) | 0.68 | 0.55 | 0.30 | 0.77 |
| Aluminium 6063-T832 [1] | tube min | 237 (0.52) | 0.68 | 0.70 | 0.39 | 0.77 |
| Aluminium 6082-T6 | tube min | 237 (0.52) | 0.69 | 0.71 | 0.39 | 0.69 |
| Aluminium 7005-T6 | typical | 244 (0.54) | 0.71 | 0.82 | 0.46 | 0.52 |
| Aluminium 7075-T6 | tube min | 246 (0.54) | 0.70 | 1.29 | 0.71 | 0.50 |
| Aluminium 7075-T73 | tube min | 246 (0.54) | 0.70 | 1.09 | 0.61 | 0.59 |
| Aluminium 7068-T6 | tube min | 250 (0.55) | 0.72 | 1.64 | 0.91 | n/a |
| Aluminium 2024-T3 | tube min | 243 (0.54) | 0.72 | 0.82 | 0.46 | 0.46 |
| Titanium grade 1 | tube min | 396 (0.87) | 1.04 | 0.39 | 0.22 | 0.08 |
| Titanium grade 2 | tube min | 396 (0.87) | 1.04 | 0.78 | 0.43 | 0.08 |
| Titanium grade 3 | tube min | 396 (0.87) | 1.04 | 1.07 | 0.60 | 0.08 |
| Titanium grade 4 | bar/plate min | 396 (0.87) | 1.02 | 1.36 | 0.75 | 0.08 |
| Titanium grade 9 annealed | tube min | 393 (0.87) | 1.02 | 1.37 | 0.76 | 0.03 |
| Titanium grade 9 CWSR | tube min | 393 (0.87) | 0.99 | 2.05 | 1.14 | 0.03 |
| Titanium grade 5 | tube min | 389 (0.86) | 1.13 | 1.98 | 1.10 | 0.03 |
| Titanium grade 23 | bar/plate min | 389 (0.86) | 1.13 | 2.15 | 1.19 | 0.03 |
| Titanium grade 12 | tube min | 396 (0.87) | 1.02 | 0.98 | 0.54 | n/a |
| Stainless 304 annealed | tube min | 693 (1.53) | 1.98 | 0.59 | 0.33 | 0.06 |
| Stainless 304 1/4-hard | strip min | 693 (1.53) | 1.98 | 1.46 | 0.81 | 0.06 |
| Stainless 316 annealed | tube min | 693 (1.53) | 1.98 | 0.59 | 0.33 | 0.06 |
| Stainless 201 annealed [2] | sheet min | 685 (1.51) | 1.95 | 0.88 | 0.49 | 0.06 |
| Stainless 430 | tube min | 679 (1.50) | 1.98 | 0.68 | 0.38 | 0.10 |
| Stainless 17-4PH H1150 | bar min | 687 (1.51) | 1.94 | 2.05 | 1.14 | 0.07 |
| Carbon steel 1020 DOM (as drawn) [3] | tube min | 689 (1.52) | 2.03 | 1.17 | 0.65 | 0.20 |
| 4130 chromoly normalized | tube min | 689 (1.52) | 2.03 | 1.46 | 0.81 | 0.16 |
| 4130 chromoly heat-treated | tube min | 689 (1.52) | 2.03 | 1.95 | 1.08 | 0.16 |
| Magnesium AZ31B tube [4] | tube min | 155 (0.34) | 0.44 | 0.31 | 0.17 | 0.37 |
| Brass C260 annealed tube | typical | 748 (1.65) | 1.09 | 0.39 | 0.22 | 0.46 |
| Copper C12200 drawn (H58, plumbing "hard") | tube min | 784 (1.73) | 1.19 | 0.58 | 0.32 | 1.30 |

### 60 in x 3/4 in OD x 0.065 in wall, every alloy

| Alloy | Strength basis | Weight g (lb) | Stiffness | Bending strength |
|---|---|---|---|---|
| Aluminium 5052-H32 | tube min | 369 (0.81) | 1.02 | 0.66 |
| Aluminium 6061-T6 | tube min | 371 (0.82) | 1.00 | 1.00 |
| Aluminium 6063-T5 | tube min | 371 (0.82) | 1.00 | 0.46 |
| Aluminium 6063-T6 | tube min | 371 (0.82) | 1.00 | 0.80 |
| Aluminium 6063-T832 [1] | tube min | 371 (0.82) | 1.00 | 1.00 |
| Aluminium 6082-T6 | tube min | 371 (0.82) | 1.02 | 1.04 |
| Aluminium 7005-T6 | typical | 382 (0.84) | 1.04 | 1.20 |
| Aluminium 7075-T6 | tube min | 385 (0.85) | 1.03 | 1.89 |
| Aluminium 7075-T73 | tube min | 385 (0.85) | 1.03 | 1.60 |
| Aluminium 7068-T6 | tube min | 392 (0.86) | 1.06 | 2.40 |
| Aluminium 2024-T3 | tube min | 381 (0.84) | 1.06 | 1.20 |
| Titanium grade 1 | tube min | 620 (1.37) | 1.52 | 0.57 |
| Titanium grade 2 | tube min | 620 (1.37) | 1.52 | 1.14 |
| Titanium grade 3 | tube min | 620 (1.37) | 1.52 | 1.58 |
| Titanium grade 4 | bar/plate min | 620 (1.37) | 1.49 | 1.99 |
| Titanium grade 9 annealed | tube min | 616 (1.36) | 1.49 | 2.00 |
| Titanium grade 9 CWSR | tube min | 616 (1.36) | 1.45 | 3.01 |
| Titanium grade 5 | tube min | 609 (1.34) | 1.65 | 2.90 |
| Titanium grade 23 | bar/plate min | 609 (1.34) | 1.65 | 3.15 |
| Titanium grade 12 | tube min | 620 (1.37) | 1.49 | 1.43 |
| Stainless 304 annealed | tube min | 1087 (2.40) | 2.90 | 0.86 |
| Stainless 304 1/4-hard | strip min | 1087 (2.40) | 2.90 | 2.15 |
| Stainless 316 annealed | tube min | 1087 (2.40) | 2.90 | 0.86 |
| Stainless 201 annealed [2] | sheet min | 1074 (2.37) | 2.86 | 1.29 |
| Stainless 430 | tube min | 1065 (2.35) | 2.90 | 1.00 |
| Stainless 17-4PH H1150 | bar min | 1077 (2.37) | 2.84 | 3.00 |
| Carbon steel 1020 DOM (as drawn) [3] | tube min | 1080 (2.38) | 2.98 | 1.72 |
| 4130 chromoly normalized | tube min | 1080 (2.38) | 2.98 | 2.15 |
| 4130 chromoly heat-treated | tube min | 1080 (2.38) | 2.98 | 2.86 |
| Magnesium AZ31B tube [4] | tube min | 243 (0.54) | 0.65 | 0.46 |
| Brass C260 annealed tube | typical | 1173 (2.59) | 1.60 | 0.57 |
| Copper C12200 drawn (H58, plumbing "hard") | tube min | 1230 (2.71) | 1.74 | 0.85 |

### Row notes

**[1] Aluminium 6063-T832.** The drawn-tube minimum depends on wall: 248
MPa for 0.025 to 0.049 inch and 241 MPa for 0.050 to 0.250 inch. Each
table uses the band for its own wall.

**[2] Stainless 201 annealed.** No tube minimum exists for 201; this row
uses the ASTM A240 sheet minimum for type 201-2. Tube is welded from
strip, and the 201-1 variant has a lower minimum. With the 201-1 minimum,
bending strength is 0.74 at 0.040 inch and 1.08 at 0.065 inch (dent index
0.41 at 0.040 inch).

**[3] Carbon steel 1020 DOM.** Retail DOM tube is sold as "1020/1026" and
is often supplied stress relieved. The row assumes as-drawn 1020 (ASTM
A513 minimum 414 MPa). Stress-relieved 1020 (379 MPa) gives 1.07 at 0.040
inch and 1.57 at 0.065 inch (dent index 0.60 at 0.040 inch). As-drawn
1026 (483 MPa) gives 1.37 and 2.00 (dent index 0.76).

**[4] Magnesium AZ31B.** Bending yields the compressed face first, and
AZ31B tube is much weaker in compression: one aerospace distributor lists
about 83 MPa compressive yield against 165 MPa tensile, both typical. The
row uses the ASTM B107 tensile minimum, so it overstates the load before a
permanent bend by about 1.3 times. With the typical compressive yield
instead, bending strength is about 0.23 at 0.040 inch and 0.34 at 0.065
inch (dent index 0.13 at 0.040 inch). No compressive minimum was found.

### What the comparison says in one paragraph

Within a family, the alloy barely changes stiffness or weight; it changes
strength. Every aluminium alloy weighs and flexes about the same in the
same tube; so does every titanium grade (grades 5 and 23 a little stiffer);
so does every steel. Across families, steel, aluminium and magnesium all
have almost the same stiffness per gram, so a heavier metal is stiffer only
because it is heavier. Titanium at 0.040 inch weighs about the same as
standard 0.065 inch aluminium and flexes about the same. The families
differ most in bending strength, dent resistance, how heat affects them,
and how much heat they carry toward your hands.

## Aluminium alloys

Aluminium is what nearly every staff maker uses: light, affordable, easy
to cut and drill. The trade-offs are that heat slowly softens its
strengthened tempers, it conducts heat toward your hands, and it dents on
hard drops (soft 6063 most of all).

**Per-alloy verdicts.** Strength figures are the 0.040 / 0.065 inch
bending strength from the tables.

| Alloy | Bending strength | Verdict |
|---|---|---|
| **6061-T6** | 0.68 / 1.00 | Fire, LED and practice. The cheap, forgiving standard. Bends before it breaks and can often be bent back |
| **7075-T6** | 1.29 / 1.89 | Fire, LED and practice. The premium flow-arts choice, roughly twice as strong as 6061. Less forgiving (a bent 7075 staff may crack if you straighten it) and more prone to corrosion and stress-corrosion cracking, so keep it anodised |
| **6082-T6** | 0.71 / 1.04 | Fire, LED and practice. The UK and European counterpart of 6061, sold in metric sizes cut to length |
| **6063-T832** | 0.70 / 1.00 | An overlooked equal of 6061-T6 (a drawn-tube temper) with better corrosion resistance and a better anodised finish, if a stockist has it |
| **6063-T6** | 0.55 / 0.80 | LED and practice; workable for fire only with a thick wall. The architectural alloy: smooth, takes anodising beautifully, and resists corrosion better than 6061 |
| **6063-T5** | 0.31 / 0.46 | LED and light practice only; takes a bend on hard drops. Much hardware-store and fencing tube is probably 6063, often unlabelled |
| **5052-H32** | 0.45 / 0.66 | LED and light practice. Tough and very corrosion resistant, but soft for a fire contact staff |
| **2024-T3** | 0.82 / 1.20 | LED and practice, possible for fire. Easy to find as aircraft tube in exact sizes, but the worst corrosion resistance of the group, so it needs anodising or paint, and paint burns at a wick |
| **7075-T73** | 1.09 / 1.60 | On paper a good fire 7075, because it is already over-aged and resists stress corrosion better. No retail staff-size tube found |
| **7005-T6** | 0.82 / 1.20 (typical, reads high) | A bicycle-frame alloy. No straight staff-size tube found for sale. Not worth chasing |
| **7068-T6** | 1.64 / 2.40 | The strongest aluminium here, but not practical: a mill order only, and the least stretchy before breaking. Heat is not what sets it apart. Its maker's data for extruded 7068, tested while held at 150 C, show yield down about 36 percent after 100 hours. Part of that is the temporary loss any aluminium shows while it is hot, which returns on cooling, and part is the permanent over-ageing every peak-aged 7xxx alloy, 7075-T6 included, is prone to. Even so, the tested extrusion kept about 414 MPa (roughly 370 MPa scaled down to the drawn-tube minimum, a computed estimate), well above new 6061-T6's 241 MPa tube minimum |

### Aluminium and wick heat

T6 aluminium gets its strength from a low-temperature bake, and time spent
hotter than that bake slowly and permanently softens it. NASA's materials
data handbooks give the bake for 6061-T6 as 171 to 182 C for about 8 hours,
and for 7075-T6 as 110 to 127 C for at least 22 hours, so 7075 starts to
lose its temper at lower temperatures. The loss depends on both
temperature and time and adds up across burns. Read by eye from a chart in
the MMPDS aerospace materials handbook (so approximate), 6061-T6 starts to
lose room-temperature strength after about 10,000 hours at 120 C or 1,000
hours at 150 C, and long exposures above about 230 to 260 C take it down
to around a fifth of its T6 yield strength, close to the fully soft
condition (ultimate strength falls less).

Even so, 7075 that has been over-aged to about T73 level stays stronger
than new 6061-T6 (1.60 against 1.00 at 0.065 inch). Heavier or longer
overheating keeps lowering it, and nobody has measured how hot staff ends
really get, so 7075 remains a reasonable fire choice as long as you check
the ends for new dents or bends. A cheap way to find out how hot your ends
run is temperature-indicating crayons or labels on the metal next to the
wick.

What that looks like in practice, over many burns: the last few inches
near the wicks may slowly get easier to dent or bend, and screw or rivet
holes may loosen. Gradual, not a sudden snap. Melting is far off: the NASA
handbooks put the start of melting at 477 C for 7075 and 582 C for 6061,
though structural strength is gone well before that.

Aluminium conducts heat well, which is why makers cover the bare metal
between grip and wick with silicone (see the fire staff guide).

**Coatings.** A trade finishing article puts the start of crazing in an
anodised layer at around 105 C, while noting that it does not always
happen at that point (**community figure**). A named polyester powder coat is cured with the metal held at
177 C, so a wick zone hotter than that re-bakes the coating it was cured
with, and expect it to discolour, crack or burn. Leave the wick zone bare,
or use a purpose-made high-temperature coating. The fume side of this is
in the heat and fumes section.

**Straightening.** 6061 and 6063 can often be bent back after a bend; 7075
and 7068 may crack instead.

## Titanium grades

Titanium changes three things at once: it keeps its strength at
temperatures that soften aluminium, it carries far less heat toward your
hands, and it costs several times more. It weighs about 1.7 times as much
as aluminium in the same tube, so the useful comparison is a thinner
titanium wall against a standard aluminium one.

### Why grade 9 is the titanium tube

Grade 9 (Ti-3Al-2.5V) was developed specifically for seamless tubing,
because grade 5 (Ti-6Al-4V) does not cold-form well. That is why grade 9
CWSR is widely stocked in thin 3/4 inch tube (bicycle frames and aircraft
hydraulic lines use it) while grade 5 tube is rare and quote-only. The big
US stockists checked listed titanium tube only in grades 2 and 9.

### Per-grade verdicts

Figures are bending strength / dent index at 0.040 inch.

| Grade | 0.040 in | Verdict |
|---|---|---|
| **Grade 9 CWSR** | 2.05 / 1.14 | **The titanium to buy.** Made in quantity as thin tube, certified to ASTM B338 or AMS specifications, and priced like grade 2 at the main small-quantity stockist |
| Grade 5 | 1.98 / 1.10 on one tube maker's minimum | A little stiffer than grade 9, but not stocked as thin tube. Bought to the usual ASTM minimum (828 MPa) it would be about 14 percent stronger than grade 9 CWSR: about 2.34 / 1.30. Either way, grade 9 CWSR is the practical buy |
| Grade 9 annealed | 1.37 / 0.76 | Good, but weaker than CWSR and, at one stockist, dearer. Only worth it if you plan to bend or form the tube |
| Grade 23 | 2.15 / 1.19 on its bar minimum | No benefit. It is grade 5 with low oxygen for implants. Its minimum (759 MPa) is below grade 5's own (828 MPa); it only looks stronger here because the grade 5 row uses one maker's lower tube figure. Sold as bar and wire, not staff tube |
| Grade 4 | 1.36 / 0.75 on a bar/plate minimum | Skip. Not sold as small tube, and it loses much of its strength when hot |
| Grade 3 | 1.07 / 0.60 | Decent on paper, but a mill-order heat-exchanger item, not sold in staff sizes |
| Grade 12 | 0.98 / 0.54 | Skip. A heat-exchanger corrosion grade between grades 2 and 3 in strength, not sold at retail |
| Grade 2 | 0.78 / 0.43 | Works but soft. Fine for LED and practice; for fire only if it is all you can get |
| Grade 7 | about grade 2 | Grade 2 plus palladium for chemical plants. Grade 2 strength at a premium. Pointless for a staff |
| Grade 1 | 0.39 / 0.22 | **Avoid.** The softest titanium there is |

**Buy with a mill certificate.** Surplus and marketplace titanium is often
sold without certification. Any honest grade 9 is fine for a staff, but a
listing for "grade 5" or plain "titanium" without a mill certificate may
be grade 9 or commercially pure. Ask for the mill test report showing the
alloy, the condition (CWSR for grade 9) and whether it is seamless or
welded.

### "What about grade 1, since it's cheaper?"

At staff size it is not cheaper, and it is the weakest titanium there is.

- No retail stockist checked had grade 1 in 3/4 inch. The main
  small-quantity stockist's only grade 1 tube was 1.375 inch OD, and
  another offered grade 1 only in 0.5, 1.25 and 1.5 inch. One supplier
  listed 3/4 inch grade 1 as quote-only.
- At retail, price follows size and how much a stockist carries, not how
  pure the metal is. At one stockist, 3/4 x 0.035 inch grade 2 and grade 9
  CWSR cost exactly the same per inch.
- Grade 1's minimum yield is 138 MPa, about half of grade 2 and about a
  fifth of grade 9 CWSR. At 0.040 inch a grade 1 tube weighs 396 g, more
  than a standard aluminium staff, yet has only 0.39 of its bending
  strength and 0.22 of its dent index. Even at 0.065 inch it reaches only
  0.57. Wick heat weakens it further. Its virtues, ductility and corrosion
  resistance, are things a staff does not need.

So in practice, the cheap titanium for a staff is grade 9 CWSR.

### Titanium and heat

- **Strength when hot.** Titanium keeps its strength at temperatures that
  soften aluminium, but the grades differ. By one maker's measurements
  (read from its chart), grade 9 CWSR keeps about 72 percent of its
  room-temperature strength at 316 C, and that maker describes grade 9 as
  usable to about 427 C before oxidation sets in. Pure grade 2 falls to
  roughly 35 to 45 percent of its room-temperature strength at 315 C.
  Grade 4 falls hardest, and its maker lists service only to 204 C.
- **Can heat undo CWSR?** Grade 9's heat treatments, from stress relief up
  to a full anneal, run at about 371 to 790 C. Long or repeated soaking of
  the tips above about 370 C could therefore slowly relax the cold work
  that makes CWSR strong. That is an inference from the heat-treatment
  range; nobody has tested it on a staff. The annealed commercially pure
  grades have no temper to lose below about 538 C.
- **Heat travel.** Titanium alloys conduct heat about 20 times less than
  6061. Computed from the tables, a 0.040 inch grade 9 or grade 5 staff
  carries roughly 35 to 40 times less heat up the tube than a standard
  aluminium staff. The flip side: the metal right next to the wick holds
  its heat, so do not grab the first inch or two just after a burn.
- **Heat tint as a rough thermometer.** The metal near the wicks will
  discolour. Light straw is surface oxide and cosmetic. A jewellery
  reference puts straw at about 385 C, purple about 412 C and deep blue
  about 440 C (**community figure**; colour also depends on how long the
  metal was hot, so treat these as rules of thumb). Straw after every burn
  on a CWSR tube suggests the tips are near the range where its cold work
  might start to relax. Purple to deep blue (about 412 to 440 C on that
  reference) means the spot went past about 400 C, the temperature above
  which The Welding Institute's guidance says oxygen and nitrogen diffuse
  into titanium, raising its strength but embrittling it. Inspect a blue
  end closely, and retire a thin wall that turns blue burn after burn.
  Light blue, grey or white (and any flaky or powdery surface) are the
  colours welders reject: that guidance treats light blue, grey and white weld
  colours as unacceptable contamination, and dark blue as acceptable only
  for some service conditions. On a thin wall, inspect such an end closely
  or retire it. Any anodised titanium colour there will change too.

### Working titanium

- **Drilling.** Slow speed, steady firm feed, sharp cobalt or carbide
  bits, and never let the bit rub without cutting. Use a chlorine-free
  cutting oil: titanium makers say to clean titanium with non-chlorinated
  solvents before it is heated. Clean off all residue away from any flame,
  and let the cleaner dry completely, because most non-chlorinated
  cleaners are highly flammable.
- **Galling.** Titanium tends to gall, so titanium-on-titanium threads
  seize. See the fire staff guide for thread and fastener choices.
- **Springback.** Titanium springs back about twice as much as stainless,
  so a bent titanium staff is hard to get true again.
- **Chips and dust burn.** A solid titanium tube is not combustible, but
  small chips, fine turnings and dust can self-ignite, and titanium powder
  auto-ignites at about 250 C. Sweep them up, keep them away from fuel and
  flame, and flush filings out of the tube before the first burn. Water on
  burning titanium can cause an explosion and carbon dioxide does not work;
  smother it with a Class D extinguisher, dry sand or dry table salt.
- **Sparks.** Titanium alloy struck hard against rock can throw sparks
  hot enough to light dry vegetation. A University of California, Irvine
  study of titanium golf club heads, published in Fire and Materials,
  found the particles struck off them make sparks that can exceed
  3,000 F long enough to ignite dry foliage (the figure is from the
  university's announcement of the paper), while stainless heads made
  none. Keep that in mind around fuel dumps, spin-off areas and dry grass.

### Do makers use titanium?

Almost no flow-arts maker publishes a titanium contact staff to compare
against. At least two brands sell titanium fire staffs, mostly spin
staffs rather than contact staffs. Fusion Arts builds its pro fire staffs on
19 mm x 140 cm titanium tube and gives 500 to 750 g complete, and
Firelovers sells a 140 cm titanium staff; neither names a grade
(**community figure**). Grade 9 CWSR comes from the bicycle-frame and
aerospace-tube world, not from flow arts.

**Cost.** On the snapshot date a 5 ft length of 3/4 x 0.039 inch grade 9
CWSR cost about $111 at the main small-quantity stockist, against $10.46
(Speedy Metals, 5 ft) to about $26.50 (Online Metals, $31.83 for 6 ft
prorated to 5 ft) for a standard 6061 tube: roughly 4 to 11 times the
price, depending on where the aluminium comes from. A marketplace listing
on Online Metals for the same titanium size was $750 for 5 ft, so shop
around.

## Stainless steel and other steels

Stainless is an excellent material for fire **hardware** and a defensible
choice for a short or heavy fire prop. For a 5 ft contact staff it is
heavy, and in the annealed form you can actually buy, it is no stronger
than aluminium.

**The good.**

- No coating is needed, so there is nothing to burn off near the wick.
- Annealed 304 and 316 have no temper to lose and keep useful strength
  hot: one producer's typical 304 yield goes from about 241 MPa at room
  temperature to about 159 MPa at 204 C and 131 MPa at 427 C.
- Low conductivity: 304 conducts about a tenth as much heat as 6061, and a
  0.040 inch stainless tube carries about 16 times less heat up the staff
  than a standard aluminium one. The flip side, inferred rather than
  measured: the metal under the wick probably runs hotter and stays hot
  longer than on aluminium.
- 304 resists fresh water, sweat and weather very well; 316 is better
  again with salt and heavy sweat.
- Very ductile, so it bends or dents rather than cracking, and can usually
  be straightened.

**The catches.**

- **Weight.** A bare 5 ft 3/4 x 0.035 inch stainless tube (about 611 g)
  already weighs nearly as much as some complete fire contact staffs.
  Sacred Flow Art lists its 7075 monkey-fist contact staff at 700 to
  850 g, and Phoenix Fire Props its 6061 contact staffs at 2 to 3.5 lb
  (about 910 to 1590 g) depending on length (**community figures**). At
  0.065 inch the bare stainless tube is 1087 g, before plugs, wicks and
  grip.
- **No stiffness per weight gain.** To match a standard aluminium staff's
  371 g, a stainless wall would have to be about 0.021 inch, which would
  dent at a look.
- **Annealed strength.** 304 and 316 tube is sold annealed, with a minimum
  yield (207 MPa, 30 ksi) a little below 6061-T6's. So a 0.040 inch
  stainless tube bends for good at about 0.59 of a standard staff's load
  despite weighing almost twice as much. Cold-worked tempers such as
  1/4-hard are far stronger on paper, but that is a strip temper and no
  round tube sold that way was found. Welded decorative tube to ASTM A554
  may be somewhat stronger than annealed from forming, but the
  specification promises no figure for it.
- **Welded tube.** A554 allows the inside weld bead to be left in, which
  can stop a plug or weight sliding in. Ask for the bead removed.
- It work-hardens when drilled (sharp bits, slow speed, steady feed), and
  stainless on stainless threads gall.

**The other stainless types.**

| Type | Bending strength at 0.040 in | Verdict |
|---|---|---|
| 304, 316 annealed | 0.59 | Great for hardware; heavy for a shaft |
| 201 annealed | 0.88 (sheet spec) | Cheaper low-nickel grade, stronger than annealed 304 but less corrosion resistant, often sold unlabelled. Carries much more manganese, which matters for welding fume |
| 430, 409 (ferritic, magnetic) | 0.68 (430) | Only slightly stronger than annealed 304 and less corrosion resistant. No reason to choose them |
| 17-4PH H1150 | 2.05 (bar spec) | Very strong, but specialty quote-only tube, and its suppliers disagree on how hot it can run: one advises against service above about 300 C, while another says its strength holds into the 316 to 427 C range |
| 321 | about 304 | A heat-resistant exhaust grade; no gain at staff temperatures |

A magnet sorts them roughly: annealed 304, 316, 321 and 201 are
essentially non-magnetic, while 430, 409 and 17-4PH are magnetic.

**Do makers use stainless shafts?** No established maker selling a
stainless-shaft contact staff was found. Stainless does appear as parts:
chains and hardware on fire poi, and forks on some fire props.

**Other steels.**

- **4130 chromoly, normalized** (1.46 at 0.040 inch) is strong and cheap as
  aircraft tube, and good for a heavy LED or practice staff if coated. It
  rusts, and the coating will burn near a wick.
- **4130, heat-treated** (1.95) is rarely sold as thin tube, a 5 ft tube
  can warp if you heat-treat it at home, and wick heat can undo the
  treatment. Not recommended.
- **1020 DOM carbon steel** (1.17) is for LED or practice only, and only
  coated. On the snapshot date it cost more than the same size in 304
  stainless.
- **Galvanised EMT electrical conduit** (1/2 inch trade size is 0.706 inch
  OD with a 0.042 inch wall) is sometimes suggested as a cheap practice
  staff. Fine for practice, but most EMT is zinc-coated steel, so keep it
  away from fire (see the fumes section).

## Other metals

- **Magnesium AZ31B** (0.31 at 0.040 inch, nearer 0.23 allowing for its
  weak compression face). **Never for fire**: magnesium alloys ignite when
  heated in air near their melting point, a thin wall catches more easily
  than a thick piece, and water on burning magnesium makes hydrogen and can
  explode. It is also weak, corrodes easily, and was found only
  quote-only from aerospace distributors, so it is a poor LED staff too.
- **Brass C260** (0.39) is heavy and soft: end weights and decorative
  ferrules only. Brass is about a third zinc, and free-machining brass
  carries about 2.5 to 3.0 percent lead; wash your hands after handling
  it.
- **Copper C12200, plumbing "hard"** (0.58, heat carried 1.30, the worst
  here). No: the heaviest option, it carries heat straight to your hands,
  and heat softens it. Also a sizing trap: US plumbing copper is named 1/8
  inch smaller than its real outside diameter.
- **Beryllium copper.** Never, anywhere on a staff. Harmless as a finished
  solid part, but the cutting, drilling, grinding and sanding you would do
  to build with it create a seriously hazardous dust.

Wood, bamboo, carbon fibre, fibreglass and plastics are covered in the
fire staff guide. In short: carbon fibre and fibreglass are good LED and
practice tube and need metal heat shields for fire; wood makes a good
practice staff and, for fire, only works with metal-covered ends.

## Which tube for which staff

A summary of the verdicts above. "Fire" means a fire contact staff; LED
and practice staffs can use anything in the fire column too.

| Use | Good choices | Avoid |
|---|---|---|
| **Fire** | 6061-T6, 6082-T6 or 7075-T6 aluminium at about 0.055 to 0.065 in; grade 9 CWSR titanium; stainless only for a short or heavy prop | Galvanised or zinc-plated steel; magnesium; copper; soft tube (grade 1 titanium, 6063-T5, and unlabelled hardware-store aluminium, probably 6063, at any wall); any painted, powder-coated or salvaged tube with an unknown finish |
| **LED** | Any aluminium above; titanium grades 2 and 9; carbon fibre; 4130 or DOM steel if you want weight | Magnesium (weak, corrodes); tube whose inside diameter will not take your LED heads |
| **Practice** | Anything above, plus hardwood dowel, 6063 and 5052, galvanised EMT, stainless for a heavy strength staff | Tube with sharp, unfinished ends |

## Heat and fumes: what heated metals and coatings give off

The short version: **the bare metal is not the problem. What is on the
metal is.**

And the biggest fume at the wick is not the staff at all. It is the
burning fuel, and once the fuel runs out, the wick itself. Put the flame
out before the wick burns dry, keep your face out of the smoke, and spin
fire outdoors. The fire staff guide and the fuels guide cover those.

### Bare metals: no meaningful fume at wick temperatures

A metal only gives off fume in quantity when it evaporates, and the
structural metals in a staff tube barely evaporate at all at a few hundred
degrees. The temperatures below are where each metal's vapour pressure
reaches a small but measurable 1 pascal, from the CRC Handbook of
Chemistry and Physics.

| Metal | Reaches 1 Pa at about | At an assumed 200 to 400 C wick zone |
|---|---|---|
| Cadmium | 257 C | **Inside the range** |
| Zinc | 337 C | **Inside the range** |
| Magnesium | 428 C | Just above it |
| Lead | 705 C | Well above |
| Aluminium | 1209 C | Far above |
| Copper | 1236 C | Far above |
| Chromium | 1383 C | Far above |
| Iron | 1455 C | Far above |
| Titanium | 1709 C | Far above |

So bare aluminium, titanium (any grade, grade 1 included), carbon steel,
stainless and copper give off no meaningful fume at wick temperatures.
The exceptions are the coating metals, zinc and cadmium, and magnesium,
whose real problem is fire rather than fume.

**Stainless and hexavalent chromium.** OSHA describes toxic hexavalent
chromium from stainless steel as a hot-work problem: welding stainless,
melting chromium, heating kiln bricks. In OSHA's words, the chromium is
"not originally hexavalent"; the high process temperatures oxidise it. No
authoritative source was found for hexavalent chromium from bare
stainless at a few hundred degrees, and a widely repeated "above 1000 F"
threshold could not be traced to any primary source. It has not been
measured on a staff either way, and that is worth saying plainly.

**7075 contains zinc,** about 5 to 6 percent, dissolved in the aluminium
under its oxide skin. It gives off far less zinc than a bare zinc coating
and is not a realistic fume source short of melting, welding or torch
cutting.

**Brass** is about a third zinc. No source gives a temperature at which
zinc starts to leave brass; zinc fume from brass is documented at brazing
and melting temperatures, and zinc alloyed in brass evaporates less
readily than pure zinc. A few brass parts near the wick are a very small
source that nobody has measured, but exposed screw heads in direct flame
run hotter than the rest of the end, so plain stainless is the better
fastener right in the flame zone.

**Titanium.** Titanium makers describe solid titanium as presenting no
hazard in the forms supplied; dust and fume come from welding, cutting and
grinding.

### The real hazards: coatings, platings and wraps

| Surface | Where you find it | When it starts to matter | What to do |
|---|---|---|---|
| **Cadmium plating** | Some aerospace, marine, military surplus and older hardware; often yellow or gold | The federal plating specification bars cadmium on parts that reach 232 C in service and warns of poisonous vapour when plated parts are welded, brazed or soldered. Cadmium reaches 1 Pa at 257 C and melts at 321 C, so wick temperatures overlap. OSHA treats heated cadmium-coated surfaces as a serious exposure | Never at the fire end. You cannot reliably tell it from yellow zinc by eye |
| **Galvanised and zinc-plated steel** | Steel EMT conduit, zinc-plated screws and washers | The galvanizers' association limits long-term service to 200 C and does not recommend above 250 C, but those are limits on the coating peeling, not fume thresholds. Zinc reaches 1 Pa at about 337 C and melts at 419 C, so direct flame at the wick edge could get there. Zinc oxide fume causes metal fume fever (chills, aches, fever), mostly from welding, cutting and brazing | A bad choice at a fire end. The often-quoted 200 C figure is the galvanizers' coating-peeling limit, not a measured fume threshold; zinc only starts to evaporate measurably around 337 C and melts at 419 C, and direct flame could reach both |
| **Yellow, gold or olive chromate** finishes | On zinc or cadmium hardware; yellow or gold "chem film" on aluminium parts | ASTM B633, the zinc electroplating standard, lists the coloured finish as a chromate (its type II) and defines the hexavalent-chromium-free alternatives separately, as passivates. The federal cadmium plating specification's chromate-treated type comes in iridescent bronze to brown, olive drab, yellow and forest green. On aluminium, the military conversion-coating specification's Type I contains hexavalent chromium (its Type II contains none). Coatings of either type range from clear to iridescent yellow, brown, gray or blue, so colour alone does not tell you which you have. Left unpainted, they start losing their protection above 60 C. The amounts are tiny and unmeasured | No reason to have them at the fire end, or to grind them. Keep chem-filmed aluminium parts out of the wick zone |
| **PTFE** (Teflon tape, PTFE-coated parts, PTFE-sealed hard anodise) | Thread tape, some fittings | The fluoropolymer industry's handling guide says hazardous fumes can form above about 330 C; overheating to 420 C releases ultra-fine particles harmful to the lungs; carbonyl fluoride and hydrogen fluoride dominate around 450 C; and the highly toxic PFIB appears above 475 C. Smoking with PTFE residue on your fingers also causes polymer fume fever | **Never use Teflon tape on fire-end threads.** Wash tape residue off your hands |
| **PVC and vinyl** | Electrical tape, vinyl wrap films | A 2024 pyrolysis study found PVC's main release of hydrogen chloride between 250 and 350 C, with benzene forming from 220 to 240 C | Keep out of the wick zone |
| **Silicone** | Self-fusing tape, silicone grips | Heat resistant, but a named silicone rubber's safety data sheet says it can form formaldehyde vapour above 150 C in air | Keep it a few inches back from the wick, off the hottest metal, and replace any that has browned or scorched |
| **Paint and powder coat** | Coloured or salvaged tube | A named polyester powder coat cures at 177 C metal temperature, below the assumed wick-zone range | Expect it to char and smoke at the wick. Leave the wick zone bare |
| **Old lead paint** | Vintage or salvaged tube | A genuine fume hazard under a flame. UK guidance keeps heat guns below 500 C on lead paint; US rules forbid open-flame burning and heat guns above 1100 F (593 C) or hot enough to char the paint. A wick flame is an open flame | Use new tube. Do not sand, grind, wire-brush, heat or torch possible lead paint off at home; test it, and treat a lead result as a reason to choose different tube |
| **Lead weights, solder, leaded brass** | End weights, old repairs | Lead fume is low at wick temperatures; lead reaches 1 Pa only at 705 C. The practical problems are melting and dripping (lead melts at about 327 C, solder lower) and lead dust on hands | No lead in a fire end. Wash hands after handling lead or free-machining brass |
| **Blue thread locker** | Threaded ends | One named blue thread locker is rated to 149 C, and its maker's release method is heating to about 250 C | Do not rely on it at a fire end |
| **Anodising** (ordinary clear or dyed) | Most coloured aluminium staffs | A trade finishing article puts the start of crazing at around 105 C, though not always at that point, so expect it to craze at the wick. No fume data was found either way, and there is very little material | Very unlikely to matter. Avoid the PTFE-sealed hard coat above |
| **Chlorinated cutting oil and cleaner residue** | Workshop leftovers | Titanium makers say to clean titanium with non-chlorinated solvents before heating it. OSHA requires chlorinated degreasing vapour to be kept away from welding, and lists phosgene among the toxic gases welding can produce | Never use chlorinated brake cleaner on anything that will be lit. Let any solvent dry completely first |

### Hazards that come only from building, not spinning

These appear when you cut, grind, weld or braze, not when you spin:

- **Welding or torch-cutting stainless** makes hexavalent chromium, nickel
  and manganese fume (201 stainless carries much more manganese).
  Ventilate or use extraction. Grinding makes metal dust rather than fume;
  wear a dust mask and eye protection.
- **Welding, cutting or brazing galvanised steel or brass** makes zinc
  oxide fume and metal fume fever.
- **Welding titanium grades 5, 9 and 23** makes dust and fume from the
  metal and its alloying elements. Grinding titanium throws hot sparks
  that can light fuel vapour.
- **Beryllium copper** is hazardous exactly when it is cut, ground,
  sanded, heat-treated or welded.
- **Fine titanium, aluminium and magnesium dust and chips** can burn.
  Sweep up, do not grind near fuel, and keep a bucket of dry sand in the
  workshop. Never water or carbon dioxide on a metal fire.

### Bottom line

Bare aluminium, stainless and titanium ends are fine from a fume
standpoint. Keep every plating, coating, tape, wrap, paint, powder coat and
thread locker out of the burn zone. Never use galvanised, cadmium-plated,
lead-painted or yellow-zinc parts at the fire end. Save the respirator and
the ventilation for cutting, grinding and welding.

## Prices, a snapshot

**Checked September 24, 2026, before shipping. A snapshot, not a
guide to what you will pay.** Long tubes can cost a lot to ship, which was
not checked.

| Tube | Where | Price on the snapshot date |
|---|---|---|
| 6061-T6, 3/4 x 0.065 in | Speedy Metals | $10.46 for 60 in; $11.51 for 72 in |
| 6061-T6, 3/4 x 0.065 in | Online Metals | $31.83 for 6 ft |
| 6061-T6, hard anodised, cut to length | Fire Mecca | $6.67 per ft, 5 ft maximum (showed sold out) |
| 6063-T832 drawn, 3/4 x 0.058 in, mill finish | Testrite Aluminum | $30.04 for 49 to 60 in |
| 7075-T6, 19 x 1.4 mm | Dark Monk | $40 listed for 72 in; the order box showed $63 with the default 1 in size selected (every size out of stock) |
| 7075-T6, 3/4 x 0.056 in | Ninja Pyrate | $30 for 64 in |
| 2024-T3, 3/4 x 0.065 in | Aircraft Spruce | $10.50 per ft (about $63 for 6 ft) |
| 4130 normalized, 3/4 in | Aircraft Spruce | $9.95/ft (0.028), $7.50/ft (0.035), $5.35/ft (0.049 and 0.065) |
| Titanium grade 2, 3/4 in | Tiger Titanium | $1.85/in (0.035), $1.75/in (0.049): about $105 to $111 for 5 ft |
| Titanium grade 9 CWSR, 3/4 x 0.035 or 0.039 in | Tiger Titanium | $1.85/in, about $111 for 5 ft |
| Titanium grade 9 annealed, 3/4 x 0.039 in | Tiger Titanium | $2.20/in, about $132 for 5 ft |
| Titanium grade 9, 3/4 x 0.039 in | Online Metals (marketplace seller Metal Mart Studio) | $750 for 5 ft (read from the listing as indexed; the page blocks automated fetching) |
| 304 annealed, 3/4 in | Speedy Metals | $43 (0.035), $25.91 (0.049), $30 (0.065), each 60 in |
| 1020/1026 DOM, 3/4 x 0.035 in | Speedy Metals | $72.73 for 60 in |
| Brass, 3/4 in | Speedy Metals | $24.61 (0.032) and $48 (0.065) for 60 in |

## You own this when

- You measured your own starting length against a wall, and tested it on
  a cheap dowel before cutting good tube.
- You can say why 3/4 inch at about 0.055 to 0.065 inch wall is the
  common contact staff tube, and what going thinner costs in dent
  resistance.
- You can check whether a plug, dowel or LED head will fit, from the
  outside diameter and the wall.
- You know that within a metal family the alloy changes strength, not
  stiffness; that across families extra stiffness costs matching extra
  weight; and that a different diameter is the efficient way to change the
  flex.
- You can explain why grade 9 CWSR is the titanium to buy, and why grade 1
  is not a bargain.
- You know which surfaces give off harmful fumes at the wick (zinc,
  cadmium, chromate, PTFE, vinyl, paint, lead paint) and which hazards
  belong only to the workshop.

When all six are true, you can choose a tube for any staff, read a tube
listing critically, and tell a marketing claim from a specification. The
same habit of asking which number is a minimum, which is typical, and
which is someone's rule of thumb works on every material you will ever
buy.

## Sources

Grouped by what kind of authority each one is. United States federal
government publications are listed first because they are in the public
domain and can be redistributed with this guide; the rest cannot, so their
facts are restated here in our own words and cited to the body that
established them. The
[research notes](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/research/2026-09-24-fire-performance/README.md)
behind this guide, with a quotation from each source, are kept in the
project's source repository, not inside the app.

### United States government (public domain)

- Occupational Safety and Health Administration. *Small Entity Compliance
  Guide for the Hexavalent Chromium Standards*, OSHA 3320-10N, 2006
  (hexavalent chromium formed by hot work; chromium not originally
  hexavalent).
  https://www.osha.gov/sites/default/files/publications/OSHA_small_entity_comp.pdf
- OSHA. 29 CFR 1910.1027 Appendix A, Cadmium (heated cadmium-coated
  surfaces as a serious exposure).
  https://www.osha.gov/laws-regs/regulations/standardnumber/1910/1910.1027AppA
- OSHA. 29 CFR 1910.252, Welding, cutting and brazing (cadmium, beryllium
  and chlorinated degreaser vapour near welding).
  https://www.osha.gov/laws-regs/regulations/standardnumber/1910/1910.252
- OSHA. *Controlling Hazardous Fume and Gases during Welding*, fact sheet
  FS-3647 (welding fume metals, coatings, phosgene).
  https://www.osha.gov/sites/default/files/publications/OSHA_FS-3647_WELDING.pdf
- OSHA. Beryllium frequently asked questions.
  https://www.osha.gov/beryllium/faqs
- National Institute for Occupational Safety and Health. NIOSH Pocket
  Guide to Chemical Hazards: Zinc oxide (metal fume fever).
  https://www.cdc.gov/niosh/npg/npgd0675.html
- Department of Housing and Urban Development. 24 CFR 35.140, Prohibited
  methods of paint removal (open flame; heat guns above 1100 F).
  https://www.law.cornell.edu/cfr/text/24/35.140
- Federal Specification QQ-P-416F, Plating, Cadmium (Electrodeposited)
  (the 450 F service limit, the vapour warning, and the colours of the
  Type II supplementary chromate treatment). Read at
  https://www.anoplex.com/references/QQ-P-416.html
- Military Specification MIL-DTL-5541F, Chemical Conversion Coatings on
  Aluminum and Aluminum Alloys (covers aluminium only; Type I contains
  hexavalent chromium and Type II contains none, section 1.2.1; coatings
  from qualified materials range from clear to iridescent yellow, brown,
  gray or blue, section 6.8, a range not tied to either type;
  unpainted coatings lose protection from 140 F). Read at
  https://www.anoplex.com/references/MIL-DTL-5541.html
- Military Specification MIL-T-6736B, Tubing, Chrome-Molybdenum (4130)
  Steel, Seamless and Welded, Aircraft Quality, 1965 (normalized and
  heat-treated 4130 tube minimums). Read as an everyspec copy at
  https://www.meracing.com/UserFiles/Products/Tubes/MIL-T-6736B_2.pdf

### Standards and specifications

- ASTM B338, seamless and welded titanium tubes (grade 1, 2, 3, 9
  annealed, 9 CWSR and 12 minimums).
- ASTM B348 and B265, titanium bar and plate (grade 23 minimum), and the
  grade 5 minimum cited for comparison.
- ASTM B210 and B483, drawn aluminium tube (5052, 6061, 6063, 7075, 7068
  and 2024 minimums), read as reprinted in Kaiser Aluminum's tube sheets.
- ASTM B221, extruded aluminium tube (6063-T5 minimum).
- ASTM A269, A213 and A554, stainless tube (304, 316 and 430 minimums;
  A554's weld-bead and condition allowances).
- ASTM A240, stainless sheet (201 minimum) and ASTM A666 (1/4-hard 304
  minimum).
- ASTM A513, electric-resistance-welded and DOM steel tube (1020 and 1026
  minimums), read as summarised by Totten Tubes.
- ASTM B107, magnesium alloy extruded tube (AZ31B minimum).
- ASTM B88, seamless copper water tube (drawn temper minimum).
- SAE AMS 4944 and 4945, titanium grade 9 CWSR tube, and AMS 5643, 17-4PH
  bar.
- ASTM B633-19, electrodeposited zinc coatings on iron and steel (type II
  coloured chromate; hexavalent-chromium-free passivates defined
  separately as types V and VI).
- BS EN 755-2, aluminium extrusions (6082-T6 minimum).
- ANSI C80.3, electrical metallic tubing (EMT dimensions). Not read
  directly: the dimensions were read from the North American EMT table in
  Wikipedia's Electrical conduit article, which does not name the
  standard. https://en.wikipedia.org/wiki/Electrical_conduit
- International Code Council. International Residential Code 2021, R305.1,
  ceiling height.
  https://codes.iccsafe.org/s/IRC2021P2/chapter-3-building-planning/IRC2021P2-Pt03-Ch03-SecR305.1

### Reference handbooks

- NASA contractor reports (written under NASA contract NAS8-26644 by
  Western Applied Research and Development, Inc. NASA's record marks them
  cleared for public use but not US government works, so they are not
  public domain; cited here for facts only):
  - NASA. Muraca, R.F. and Whittick, J.S., *Materials Data Handbook:
    Aluminum Alloy 6061*, NASA-CR-123772, 1972 (the T6 ageing treatment and
    the melting range).
    https://ntrs.nasa.gov/api/citations/19720022808/downloads/19720022808.pdf
  - NASA. Muraca, R.F. and Whittick, J.S., *Materials Data Handbook:
    Aluminum Alloy 7075*, NASA-CR-123773, 1972 (the T6 ageing treatment and
    the melting range).
    https://ntrs.nasa.gov/api/citations/19720022809/downloads/19720022809.pdf
- CRC Handbook of Chemistry and Physics, 84th edition, vapour pressure of
  the metallic elements (the 1 Pa temperatures) and melting points. Read
  as transcribed on Wikipedia's data page,
  https://en.wikipedia.org/wiki/Vapor_pressures_of_the_elements_(data_page)
- MMPDS-04, *Metallic Materials Properties Development and
  Standardization*, 2008, the figure for the effect of exposure at
  elevated temperature on 6061-T6 (read by eye).

### Peer-reviewed literature

- Arulmoli, Earthman and others, *Spark production by abrasion of
  titanium alloys in golf club heads*, Fire and Materials, 2015 (titanium
  alloy particles produced on abrasion).
  https://onlinelibrary.wiley.com/doi/abs/10.1002/fam.2235. The 3,000 F
  figure, the dry-foliage ignition and the stainless heads making no
  sparks are from the University of California, Irvine's announcement of
  the paper, *Playing with fire*, UCI News, April 10, 2014 (the paper's
  full text was not read).
  https://news.uci.edu/2014/04/10/playing-with-fire/
- Wu, J., Papanikolaou, K.G., Cheng, F., Addison, B., Cuthbertson, A.A.,
  Mavrikakis, M. and Huber, G.W., *Kinetic Study of Polyvinyl Chloride
  Pyrolysis with Characterization of Dehydrochlorinated PVC*, ACS
  Sustainable Chemistry and Engineering 12(19), 2024 (hydrogen chloride
  release at 250 to 350 C; benzene from 220 to 240 C).
  https://www.osti.gov/biblio/2352421
- Fromuth, R.C. and Parkinson, M.B., *Predicting 5th and 95th percentile
  anthropometric segment lengths from population stature*, Proceedings of
  ASME IDETC/CIE 2008 (the body-proportion ratios are ambiguously defined
  and unvalidated).
  https://asmedigitalcollection.asme.org/IDETC-CIE/proceedings-abstract/IDETC-CIE2008/43253/581/330839

### Manufacturer and industry technical literature (named documents)

- Kaiser Aluminum. Tube and pipe data sheets for 5052, 6061, 6063, 7075
  and 2024, and the hard alloy drawn seamless tube capabilities sheet
  (drawn-tube minimums, densities, moduli, conductivities, corrosion and
  cold-workability ratings).
  https://online.kaiseraluminum.com/depot/PublicProductInformation/Document/1009/Kaiser_Aluminum_Hard_Alloy_Drawn_Seamless_Tube_Capabilities.pdf
- Kaiser Aluminum. *Alloy 7068* brochure (7068-T6511 yield measured at
  temperature after exposures up to 100 hours at 100 to 175 C, on a
  1.75 inch extrusion with about 645 MPa yield at room temperature, read
  from the charts; modulus).
  https://online.kaiseraluminum.com/depot/PublicProductInformation/Document/1033/Kaiser_Aluminum_Alloy_7068_Brochure.pdf
- Alleima. *Alleima Ti Grade 9, tube and pipe, seamless* datasheet,
  updated May 9, 2025 (modulus, strength at temperature, usable to about
  427 C, springback about twice that of stainless).
  https://www.alleima.com/en/technical-center/material-datasheets/tube-and-pipe-seamless/sandvik-ti-grade-9/
- Alleima. *Safety Information Sheet for Titanium Grade 9*, November 2023
  (massive titanium not combustible; fines self-ignite; powder
  auto-ignition 250 C; no water, no carbon dioxide).
  https://www.alleima.com/contentassets/d9bd874a53914c4e804963021ac88f7b/titanium-grade-9.pdf
- Haynes International. Ti-3Al-2.5V brochure (developed for seamless
  tubing; heat-treatment range; standard walls).
  https://haynesintl.com/wp-content/uploads/2024/08/ti-3al-2-5v-brochure.pdf
- Fine Tubes (AMETEK). Titanium alloy data sheets for Ti-6Al-4V grade 5
  and Ti-3Al-2.5V grade 9 (the grade 5 tube minimum; grade 9 conductivity).
  https://www.finetubes.co.uk/-/media/ametekfinetubes/files/downloads/alloy-data-sheets/titanium---alloy-6al4v-grade-5.pdf
- TIMET. *Titanium Design and Fabrication Handbook* (clean with
  non-chlorinated solvents before heating) and the TIMETAL grade 12
  datasheet.
  https://www.timet.com/assets/local/documents/technicalmanuals/DesignandFabrication.pdf
- Zapp. Titanium Grades 1 to 4 datasheet (conductivity; grade 2 strength
  at 315 C).
  https://www.zapp.com/fileadmin/_documents/Downloads/materials/high_performance_alloys/en/Titanium-Grade-1-4_Datasheet.pdf
- Carpenter Technology. CP Titanium Grade 4 datasheet (bar/plate minimum,
  hot strength, service limit, galling).
  https://www.carpentertechnology.com/hubfs/Data%20Sheets/CP_Ti_Grade_4_Datasheet.pdf
- Rolled Alloys. 304/304L stainless data sheet (strength at 204 and
  427 C, density, modulus).
  https://www.rolledalloys.com/wp-content/uploads/304-304L_stainless-steel-data-sheet-rolled-alloys.pdf
- Sandmeyer Steel. Alloy 17-4PH data (not to be used above 300 C).
  https://www.sandmeyersteel.com/17-4PH.html
- SSA Corp. *17-4 Stainless Steel, AMS 5643, AISI 630* data sheet
  (properties maintained in service up to the 600 to 800 F range).
  https://www.ssa-corp.com/documents/Data%20Sheet%2017-4-Stainless-Steel-AMS-5643-UNSS17400.pdf
- AK Steel. Type 201 stainless data sheet (density, modulus,
  conductivity).
  https://www.spacematdb.com/spacemat/manudatasheets/201_Data_Sheet.pdf
- Copper Development Association. Alloy data for C12200, C26000 and
  C36000 (copper and brass properties; lead in free-machining brass).
  https://alloys.copper.org/alloy/C26000
- Materion. Safety data sheet, Copper Beryllium Wrought Alloy, version 09,
  2025 (hazard during cutting, grinding, heat treating and welding).
  https://www.materion.com/en/resources/environmental-health-safety/safety-data-sheets/DownloadSds?sdsId=A10_COPPER+BERYLLIUM+WROUGHT+ALLOY+_SDS-US_English.pdf
- TW Metals. Magnesium safety data sheet, 2022 (ignition near the melting
  point; water on burning magnesium).
  https://www.twmetals.com/media/category_pdfs/SDS-Magnesium.pdf
- Dow. *DOW CORNING C6-540 Liquid Silicone Rubber* safety data sheet,
  2018 (formaldehyde vapour above 150 C in air).
  https://b2b-safety-data-sheets.s3.us-east-2.amazonaws.com/DuPont+Silicones+(formerly+DDP,+Dow+Corning)/Liquid+Silicone+Rubber/DS_LSR+C6-540+Parts+A-B_EN.pdf
- Henkel. *LOCTITE Threadlocker Blue 242* technical data sheet (service
  to 149 C; heat to 250 C to release).
  https://datasheets.tdx.henkel.com/LOCTITE-THREADLOCKER-BLUE-242-en_US.pdf
- Cardinal Industrial Finishes. Technical data sheet C241-BK109,
  polyester powder coating (cure 10 minutes at 350 F metal temperature).
  http://www.cardinalpaint.com/assets/Uploads/BK109-C241-TDS.pdf
- Plastics Industry Association, Fluoropolymers Division. *Guide to the
  Safe Handling of Fluoropolymer Resins*, fifth edition (PTFE
  decomposition temperatures; polymer fume fever from contaminated
  tobacco).
  https://www.theic2.org/wp-content/uploads/2023/05/Guide-to-the-Safe-Handling-of-Fluoropolymer-Resins-v5-20190130-1.pdf
- American Galvanizers Association. Hot-dip galvanizing in extreme
  temperatures (200 C and 250 C service limits) and welding galvanized
  steel (zinc melting point).
  https://galvanizeit.org/hot-dip-galvanizing/how-long-does-hdg-last/in-extreme-temperatures
- TWI (The Welding Institute). Mathers, G., *Welding of titanium and its
  alloys, part 1*, Job Knowledge 109 (oxygen and nitrogen diffusing into
  titanium above about 400 C and embrittling it; weld colours from
  acceptable straw, through dark blue, acceptable only for some service
  conditions, to unacceptable light blue, grey and white).
  https://www.twi-global.com/technical-knowledge/job-knowledge/welding-of-titanium-and-its-alloys-part-1-109

### Fire departments and other government bodies

- Portland Fire & Rescue. Fire Marshal's Office policy FIR 3.07, *Fire
  Performance Art* (sprinklers and a 12 ft minimum ceiling for indoor fire
  performance venues).
  https://www.portland.gov/sites/default/files/2020-06/fir-3.07-fire-performance-art-080218-755010.pdf
- UK Health and Safety Executive. Lead in construction (keep heat guns
  below 500 C on lead paint).
  https://www.hse.gov.uk/construction/healthrisks/hazardous-substances/lead.htm

### Not authorities, but useful for one practical detail

Figures from staff makers, teachers and retailers are labelled
**community figure** in the text, and prices and stock are a snapshot
checked September 24, 2026. The reference sites at the end of this list
each supply a single textbook or database value, and the text says where
it uses one.

- Staff makers and teachers, for length rules, diameters, walls in use,
  and complete staff weights: Flow Arts Institute
  (https://flowartsinstitute.com/tips-for-first-staff/), Bonobo Flow
  (https://bonoboflow.com/how-to-choose-the-perfect-staff-length/),
  Dark Monk (https://dark-monk.com/Article/Sizing-Contact-Staff,
  https://dark-monk.com/Equipment/Aluminum-Tubing, for the 5/8 inch dowel
  fit, and https://dark-monk.com/Equipment/Carbon-Fiber-Tube, for its
  2 mm carbon fibre wall), Fire Mecca
  (https://firemecca.com/products/contact-fire-staff and, for its tube
  price and 5 ft maximum, https://firemecca.com/products/aluminum-staff-tubing),
  RandyLeeSticks (https://randyleesticks.com/sizing-a-contact-staff/, also
  for 5/8 inch tube under about 5 ft 4 in), Master Flow Arts
  (https://www.masterflowarts.com/contact-staff/choosing-a-contact-staff),
  Home of Poi (https://www.homeofpoi.com/us/help/faq/3/159), Fire and
  Flow NZ (https://fireandflow.co.nz/blogs/resources/size-guide, for the
  double-staff torso and arm test), NeoFlowArt
  (https://neoflowart.com/blog/cstaff-vs-staff/), Phoenix Fire Props
  (https://phoenix-fireprops.com/product/the-full-package-contact-fire-staff/,
  for the height chart and its 2 to 3.5 lb staff weights), Firetoys
  (https://www.firetoys.com/blogs/fire/how-to-choose-a-staff-for-spinning),
  Threeworlds (https://www.threeworlds.com.au/products/isis-pro-fire-staff),
  Ninja Pyrate (https://ninjapyrate.com/7075-t6-aluminum-tubing/), Buy
  Fire Fans (https://buyfirefans.com/product/7075-t6-aluminum-fire-staff-tubing/),
  and Flowtoys, for LED capsule and tube sizes
  (https://flowtoys.com/products/capsule-light-2c).
- Sacred Flow Art, for its monkey-fist contact staff weight of 700 to
  850 g (https://sacredflowart.com/product/contact-fire-staff/) and its
  dragon staff sizing by wick count
  (https://sacredflowart.com/product/fire-dragon-staff/).
- Blanketfort, for a double staff reaching about mid upper arm when held
  at its centre. https://www.blanketfort.com/juggling/staff.html
- Renegade Juggling, a US maker, for a 20 mm 6063 staff
  (https://renegadejuggling.com/products/aluminum-fire-staff-eight-inch-wicks;
  based in Santa Cruz, California, per
  https://renegadejuggling.com/pages/contact).
- The Spinsterz, a US retailer, for a 7/8 inch (22 mm) Fusion contact
  staff made by Threeworlds; the product page carries Threeworlds' own
  maker copy and names the patented Fusion locking mechanism
  (https://thespinsterz.com/products/fusion-fire-contact-staff; its
  shipping page says orders go out from Colorado,
  https://thespinsterz.com/pages/shipping-info). Threeworlds, listed
  above, is the Australian maker, and uses both 20 mm (the Isis Pro page)
  and 22 mm (the Fusion system).
- The Flowbonacci dragon staff, for a 1.5 mm carbon fibre wall
  (https://flowtoys.com/flowbonacci-dragon-staff).
- Titanium fire staffs on sale: Fusion Arts, for 19 mm x 140 cm titanium
  tube and 500 to 750 g complete
  (https://fusion-arts.com/choose-fire-staff/), and Firelovers, via
  Oddballs, for a 140 cm titanium staff
  (https://www.oddballs.co.uk/products/firelovers-titanium-fire-staff-140cm).
- Metal stockists, for prices, stock sizes and some product weights:
  Speedy Metals (https://www.speedymetals.com/c-8371-round-tube.aspx),
  Online Metals (https://www.onlinemetals.com/, and for the grade 9
  marketplace listing
  https://www.onlinemetals.com/en/buy/titanium/3-4-inch-od-x-0-039-inch-wall-titanium-round-tube-grade-9-3al-2-5v/pid/mp-00067551),
  Tiger Titanium (https://www.tigertitanium.com/storek/Tube), Aircraft
  Spruce (https://www.aircraftspruce.com/catalog/mepages/4130tubing_un1.php),
  Testrite Aluminum
  (https://www.testritealuminum.com/products/3-4-od-x-058-wall-round-aluminum-tubing-drawn-telescopic-compatible-6063-t832),
  KIMetals
  (https://kimetals.co.uk/materials/aluminium/aluminium-round-tubes/kim144773),
  McMaster-Carr (https://www.mcmaster.com/products/titanium-tubing/), and
  Totten Tubes' summary of ASTM A513
  (https://www.tottentubes.com/astm-a513-specification-information).
- Titanium stockists, for grade 1 availability: TMS Titanium, whose grade
  1 tube was 0.5, 1.25 and 1.5 inch only
  (https://store.tmstitanium.com/products/titanium-tubing/cp-grade-1/),
  and SAM Materials (Stanford Advanced Materials), which listed 3/4 inch
  grade 1 as quote-only
  (https://www.samaterials.com/item/tm7273-titanium-tube-ti-tube-grade-1-ta1-od-34-inch.html).
- BTI Metals, an aerospace distributor, for the typical compressive and
  tensile yield of AZ31B tube.
  https://btimetals.com/aerospace_metal/magnesium-az31b-tubes/
- MatWeb, for the Aluminum Association typical properties of 7005-T6.
- Products Finishing magazine, Chesterfield, L., *Cracking of the Anodic
  Coating Under High Temperatures*, February 18, 2011 (crazing at around
  105 C, though not always at that point).
  https://www.pfonline.com/articles/cracking-of-the-anodic-coating-under-high-temperatures
- Untracht, O., *Jewelry Concepts and Technology*, 1982, as cited on
  Wikipedia, for titanium heat-tint colours and temperatures.
- HyperPhysics, Georgia State University, for the moment of inertia of a
  rod (the cube-of-length result).
  http://hyperphysics.gsu.edu/hbase/mi2.html
- RoyMech ergonomics tables, for average overhead reach in the ceiling
  arithmetic.
  https://www.roymech.co.uk/Useful_Tables/Ergonomics/Human_sizes.html
