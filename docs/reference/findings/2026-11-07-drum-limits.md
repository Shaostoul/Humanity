# How big can a spinning drum be, how long, and how steep may its floor lean? Limits for the mothership

**Research date: 2026-11-07.** Every source below was read on 2026-11-07. This is a reading of
public sources, done so the game's ship is sized by real limits. **It is not engineering advice**,
and nobody who wrote it is a structural or aerospace engineer. Anyone designing a real structure
should go to the primary sources and to a professional.

**Note added 2026-11-13: one input moved; the readings below are unchanged.** The shield this
finding computed with, 4.5 t/m2 (the project's closed-loop figure, from NASA SP-413), was corrected
by [the mothership sizes research](../research/2026-11-13-mothership-sizes/README.md), section 7:
by NASA's OLTARIS tool (Globus and Strout 2017) it gives about 55 mSv a year behind water, not 5.
The operator chose 5 mSv a year, about 10 t/m2 of water
([master plan 1.5](../../design/mothership-master-plan.md), item 8). The same scale study, rerun
on 2026-11-13: a billion people need about 5.22 x 10^12 t (about 5,220 t a person, two millionths
of the main belt, one rock about 17 km across), and a 1 km drum's shield (8.5 x 10^7 t) outweighs
its carbon rim about 75 times. The limits (hoop stress, spin stability, the spine's length,
buckling) and the ship counts do not depend on the shield and stand. The masses below are as
computed on 2026-11-07, at 4.5 t/m2.

## The question

What physical limits decide how large a 1 g spinning drum can be (hoop stress for steel, aramid and
carbon composites), how long it may be before its spin turns unstable, what loads its non-rotating
spine carries, how much material a billion people's drums take and whether the asteroid belt holds
it, and how steep a floor people walk without treating it as a slope?

Asked by the operator on 2026-11-07, verbatim: "Let's aim for 1 billion and 10 billion." And on the
floor's lean while the drive pushes: "I'm wondering how we'd design a drum design that has
subsections that are able to adjust to either one or two axis. Gyroscopes start add a lot of excess
structure." The design that uses the answers is
[mothership-master-plan.md](../../design/mothership-master-plan.md), sections 3.14, 3.15 and 3.16,
through `node scripts/space-program.js --sector` and `--scale`.

## Short answer

- **Hoop stress sets the widest drum.** A spinning rim at 1 g carries its own weight as a stress of
  density x g x radius (F1), plus the air and contents pressing on it. With a safety factor of 2
  (a Proposal), no steel drum can be wider than about 4.5 km even empty; aramid composite about
  63 km; carbon fibre composite about 83 km (computed from F3 to F5). At the 250 m of Drum 1 a
  carbon composite rim is about 2 cm thick; at 1 km about 9 cm.
- **Spin stability sets the longest drum.** A cylinder flying free spins stably about its long axis
  only when that axis has at least 1.2 times the moment of inertia of the others, which for a thin
  shell with end caps means **no longer than about 1.3 radii** (F2, reproducing Globus et al.
  2007). Kalpana One, a 250 m radius settlement like Drum 1, was cut to 325 m long for exactly
  this reason (F2, F6). **Drum 1 is 2,000 m long, 8 radii**: as a free body it would not be
  passively stable. Active control (moving weights or water) is the alternative the sources name
  (F2).
- **The shield dominates the mass**, as Jensen also found (F1): at 4.5 t of shield per m2 of hull
  (the project's closed-loop figure) a drum's shield outweighs its carbon rim about thirty times.
- **A billion people need about 2.65 x 10^12 tonnes; ten billion ten times that** (computed by the
  space program's scale study). The main asteroid belt holds about 2.4 x 10^21 kg (F7): a
  billion people's ships are about a millionth of it, the mass of one rock about 14 km across.
- **One ship cannot hold them; a fleet can** (computed, a judgement on the criterion): a spine
  carrying 1 km drums for a billion people would be 67,500 km long, and holding its own
  manoeuvring loads it would weigh many times what it carries; ships of about 330 km (246 drums,
  4.9 million people) keep the spine near a tenth of the ship's mass.
- **Floors:** the US accessibility standard lets an ordinary walking surface slope at most 1 in 20
  (2.86 degrees), calls anything steeper a ramp, and allows a cross slope of 1 in 48 (1.19 degrees)
  (F8). A drive pushing at 0.05 g leans "down" by exactly 1 in 20; at 0.02 g by 1 in 50, flatter
  than the cross slope allowed on any walkway.

---

## How the sources were read

- arXiv preprints were read from their abstract pages and their PDFs, the PDFs turned into text
  with `pdftotext -layout`. **The text extraction drops Greek letters**: where a quotation below
  needs one, it is restored in square brackets, [σ] or [ρ], and nothing else is changed.
- The two Jensen papers are preprints (arXiv), not peer-reviewed journal articles. They are used
  because they set out the stability rule and the hoop stress formula with their own analysis and
  credit the earlier work (Globus et al. 2007, whose own copy could not be reached).
- Manufacturers' data sheets are the source for material strengths; each says its values are
  typical, for selection. A real design would use guaranteed specification values.

## Findings

### F1. Jensen 2024a: the rim's own weight is its largest stress, and the shield dominates the mass

David W. Jensen, "Design Limits on Large Space Stations", arXiv:2408.00152, v1 submitted 31 July
2024 ([arxiv.org/abs/2408.00152](https://arxiv.org/abs/2408.00152)), section 3.4.5:

> "The shell centripetal hoop stress, [σ]c, is the largest and is equal to [ρ]gR where g is the
> centripetal gravity, [ρ] is the material density, and R is the rotation radius. In this design,
> the circumferential air pressure, [σ]a, is the next largest and is equal to P R/t where P is the
> internal air pressure, t is the shell thickness, and R is the rotation radius."

On shielding, section 3.4.4:

> "They considered both attached shielding, which is integral to the design, and unattached
> stationary shielding, which has a separate living area rotating inside."

("They" is Bock, Lambrou and Simon's 1979 NASA paper, which this pass did not read.) And:

> "Even for thin-shell single-floor designs, one quickly finds that shielding mass dominates the
> station structural mass"

On size: "All the materials support their working stress below the rotation radius of 10,000
meters." (section 3.4.5, of the four materials in his chart).

### F2. Jensen 2024b: the 1.2 inertia ratio, and a cylinder no longer than 1.3 radii

David W. Jensen, "Space Station Rotational Stability", arXiv:2408.00155, v1 submitted 31 July 2024
([arxiv.org/abs/2408.00155](https://arxiv.org/abs/2408.00155)). Section 4.1 quotes the Kalpana One
paper (Globus et al. 2007):

> "Experience with spin-stabilized spacecraft suggests that the desired axis of rotation should have
> an angular moment of inertia at least 1.2 times greater than any other axis."

> "If that axis is not along the cylinder length, this introduces a catastrophic failure mode where
> the settlement gradually changes its rotational axis until it is tumbling end-over-end."

Section 4.4, his own result:

> "That analysis finds the cylinder length over the cylinder radius (L/R) should be 1.3 to be stable
> (Iz=1.2Ix). This analysis and result match the Globus study [Globus et al. 2007]."

Section 4.2, on the long O'Neill cylinders and on active control:

> "The researchers noted that the Bernal Sphere and the O'Neill Cylinder would be unstable because
> their desired axis of rotation was not 1.2 times greater than the other axes."

> "Ultimately, station designs will use computers and accelerometers to provide active control and
> counteract such instabilities. Authors have suggested active control techniques using motors to
> move large weights on cables [Globus et al. 2007] or using pumps to move water [Lipsett 2005]."

### F3. High-strength steel: 700 MPa

SSAB, "Strenx® P700" product page, no date shown
([ssab.com/en/brands-and-products/strenx/product-offer/700/p700](https://www.ssab.com/en/brands-and-products/strenx/product-offer/700/p700)):

> "Strenx® P700 is a pressure vessel steel with minimum yield strength of 670 - 700 MPa depending
> on thickness."

Its table gives 700 MPa for plate 4.0 to 50.0 mm thick and a tensile strength of 770 to 940 MPa.
The density used, 7,850 kg/m3, is the usual figure for carbon steel and was **not** read from a
primary source in this pass.

### F4. Aramid fibre (Kevlar 49): 2,760 MPa at 1.44 g/cm3

Goodfellow, "Kevlar 49 Polyaramid Fiber | PPTA | CAS 25035-37-4", product page, no date shown
([goodfellow.com/usa/kevlar-49-polyaramid-fibre-group](https://www.goodfellow.com/usa/kevlar-49-polyaramid-fibre-group)):
"Tensile strength( MPa )" "2760"; "Density( gcm⁻³ )" "1.44". A composite at 60% fibre by volume
is taken as 0.6 x 2,760 = 1,656 MPa (the fibres carry the load) at 0.6 x 1,440 + 0.4 x 1,200 =
1,344 kg/m3, the resin's density assumed (computed, a Proposal).

### F5. Carbon fibre composite (Toray T700S): 2,550 MPa, 135 GPa

Toray Carbon Fibers America, "T700S Data Sheet", No. CFA-005 (no date printed; the PDF's own
metadata says November 2002), read from the copy hosted by Oak Ridge National Laboratory
([thyme.ornl.gov/CFCrush/materials/cmh17/T700SDataSheet.pdf](https://thyme.ornl.gov/CFCrush/materials/cmh17/T700SDataSheet.pdf)).
Fibre: "711 ksi 4,900 MPa", density "1.80 g/cm3". Composite: "Tensile Strength 370 ksi 2,550 MPa",
"Tensile Modulus 20.0 Msi 135 GPa", with the note "* Toray 250°F Epoxy Resin. Normalized to 60%
fiber volume." and "These values are for material selection purposes only." The composite's
density, 1,560 kg/m3, is 0.6 x 1,800 + 0.4 x an assumed resin of 1,200 (computed).

### F6. Kalpana One: a 250 m radius, 325 m long

National Space Society, "Kalpana One Space Settlement", no date shown
([settlement.nss.org/Kalpana/KalpanaOne.html](https://settlement.nss.org/Kalpana/KalpanaOne.html)):

> "The Kalpana One structure is a cylinder with a radius of 250m and a length of 325m."

> "The population target is 3,000 residents."

325 m is 1.3 times 250 m.

### F7. The main asteroid belt: about 4.0 x 10^-4 Earth masses

E. V. Pitjeva and N. P. Pitjev, "Masses of the Main Asteroid Belt and the Kuiper Belt from the
Motions of Planets and Spacecraft", *Astronomy Letters* 44: 554-566 (2018), arXiv:1811.05191,
submitted 13 November 2018 ([arxiv.org/abs/1811.05191](https://arxiv.org/abs/1811.05191)):

> "Our mass estimate for the main asteroid belt is (4.008 ± 0.029)·10^-4 m⊕ (3σ)."

Times 5.972 x 10^24 kg for the Earth: about 2.394 x 10^21 kg (computed).

### F8. How steep a floor may be: the 2010 ADA Standards

US Department of Justice, "2010 ADA Standards for Accessible Design", September 15, 2010
([ada.gov/law-and-regs/design-standards/2010-stds/](https://www.ada.gov/law-and-regs/design-standards/2010-stds/)):

> "Ramp. A walking surface that has a running slope steeper than 1:20."

> "403.3 Slope. The running slope of walking surfaces shall not be steeper than 1:20. The cross
> slope of walking surfaces shall not be steeper than 1:48."

> "405.2 Slope. Ramp runs shall have a running slope not steeper than 1:12."

1:20 is 2.86 degrees, 1:48 is 1.19 degrees, 1:12 is 4.76 degrees (computed).

## Computations (the space program's scale study, 2026-11-07)

`node scripts/space-program.js --scale` works these from `data/ship/space_program.toml`; the
formulas are F1's and the textbook ones named below.

| Material (working strength = strength / 2) | Widest drum, empty (sigma / rho g) | Rim at 250 m | Rim at 1 km |
|---|---|---|---|
| Steel, 350 MPa, 7,850 kg/m3 | 4.5 km | 8.4 cm, 659 kg/m2 | 40.7 cm, 3,195 kg/m2 |
| Aramid composite, 828 MPa, 1,344 kg/m3 | 62.8 km | 3.4 cm, 45 kg/m2 | 13.6 cm, 183 kg/m2 |
| Carbon composite, 1,275 MPa, 1,560 kg/m3 | 83.3 km | 2.2 cm, 34 kg/m2 | 8.8 cm, 138 kg/m2 |

The rim's wall is t = q R / (sigma - rho g R), with q the air (101.3 kPa, assumed) plus 1 t/m2 of
contents (a Proposal), both pressing outward at 1 g.

- **Spin stability (F2):** at 250 m radius a free drum may be 325 m long; at 1 km, 1,300 m. Drum 1
  is 2,000 m: 8.0 radii.
- **The spine:** pulled or pushed at acceleration a, it carries everything ahead of or behind the
  drive, so its section grows along it and its mass is a share rho a L / (2 sigma) of what it
  carries (computed, leaving out the spine's own weight). Held to a tenth, a carbon composite spine
  at 0.05 g may be 333 km long.
- **Buckling:** pushed from the stern, that spine is a column. Euler's formula, P = pi^2 E I / L^2,
  for a thin tube of 60 m radius sized to carry a 332 km ship at 0.05 g, allows about 1.4 km
  between braces (computed with F5's modulus). So a long ship is pulled from the bow, or each drum
  carries its own thrusters.
- **Mass:** a 1 km drum of 1,300 m: rim 1.1 x 10^6 t, end walls 0.8 x 10^6 t, contents 8.2 x
  10^6 t, shield (non-rotating tube over one drum and its gap) 3.8 x 10^7 t: about 2,650 t a person
  with the spine's tenth.

## What is still unknown, and what would settle it

| Unknown | What would settle it | Cost |
|---|---|---|
| Whether a long drum held on bearings around a non-rotating spine (a "dual-spin" body) escapes the free-body 1.3-radius rule, and under what damping | Likins, "Attitude Stability Criteria for Dual Spin Spacecraft", J. Spacecraft and Rockets 4: 1638-1643 (1967), and its successors, read in full | a library copy or the AIAA paper |
| The structure allowance for floors, beds and buildings on the rim (the 1 t/m2 here) | a mass breakdown from NASA SP-413's structures chapter, read for this question | an hour with the 1977 study |
| The density of the steel and the resin, here assumed | a manufacturer's data sheet giving density | minutes |
| A shield thinner than 4.5 t/m2 | the radiation finding the master plan still owes | a research pass (done 2026-11-13: the shield must be thicker, about 10 t/m2 of water for 5 mSv a year; the note at the top) |
| How fast people tolerate a drum spinning (Drum 1 at 1.89 rpm against SP-413's 1 rpm) | the rotation-tolerance studies the master plan names in 3.13 | a research pass |

## Sources that could not be reached

- `alglobus.net/NASAwork/papers/2007KalpanaOne.pdf` (the Kalpana One paper itself): the site's
  certificate did not match its name, so the request was refused. Its words are quoted here as
  Jensen reproduces them (F2).
- Toray's own copies of the T700S sheet (`toraycma.com/wp-content/uploads/T700S-Data-Sheet-1.pdf`
  and the April 2025 sheet): 404. The ORNL-hosted copy was read instead.
- DuPont's Kevlar Technical Guide (`dupont.com/.../Kevlar_Technical_Guide_0319.pdf`): 404.
- SSAB's Strenx 700 E/F page on `websitecdn.ssab.com`: the name did not resolve; the P700 page was
  read instead.
- Likins 1967 (dual-spin stability): paywalled at AIAA; a NASA report on the subject
  (ntrs.nasa.gov 19710018130) and a Naval Postgraduate School thesis are scans whose text could not
  be read.

## What it means for this project

### Certain (from the sources)

- A free-flying cylinder at Drum 1's radius is passively stable up to about 325 m long (F2, F6);
  Drum 1 at 2,000 m is not, as a free body.
- Steel cannot make a 1 g drum much wider than a few kilometres; aramid and carbon composites
  allow tens of kilometres (F3 to F5 with F1's formula).
- The shield, not the structure, is most of the mass (F1).
- 1 in 20 is the steepest floor the ADA lets people walk without calling it a ramp; 1 in 48 the
  steepest cross slope (F8).

### Judgement calls (ours)

- Build drums no longer than 1.3 radii, so each one would stay stable even flying free, and give
  the whole ship active damping as well; and ask the operator whether Drum 1's 2 km becomes a row
  of short drums (master plan question 1b).
- Hold the shield in a non-rotating shell around the drums, so the drums carry no shield and the
  shield carries no spin (F1's "unattached" option).
- Cap manoeuvring thrust at 0.05 g, routinely 0.02 g, so a floor never leans past what people walk
  daily (F8), and no part of the floor has to move.
- A ship of about 330 km and a fleet of them, not one ship: the criterion (the spine at a tenth of
  the mass) is a Proposal; a stricter or looser one moves the number of ships, never the
  conclusion that one spine cannot hold a billion people.

This is a reading of public sources, not engineering advice; nobody who wrote it is an engineer.
