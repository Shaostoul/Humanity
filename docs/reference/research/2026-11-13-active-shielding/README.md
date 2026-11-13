# Shielding the mothership: is there a better way than 10 t/m2 of water? (magnets included)

**Research date: 2026-11-13.** Every source was read on 2026-11-13; each is listed with its own
date and the words that carry the meaning in the helper file [sources.md](sources.md) (cited here
as S1, S2 ...). Numbers from the [mothership sizes research](../2026-11-13-mothership-sizes/README.md)
keep its labels (F28 and so on, in its [helper file F](../2026-11-13-mothership-sizes/F-structure-and-shielding.md)).
Anything not quoted from a source is marked **computed** (arithmetic on sourced or decided numbers)
or **estimate** (a judgement). This is a reading of public sources for a game and a real-life
plan; it is not engineering advice, and nobody who wrote it is a radiation physicist.

The operator, 2026-11-13, on the decided shield (5 mSv a year, about 10 t/m2 of water, master plan
1.5 item 8 and 3.16): "That sounds like a lot of water, how thick is the drum going to be just for
the steel and water? Is there a better way to do it? What about magnetic shielding? Like generating
a magnetic field to protect residents? Kinda like how the Earth magnetic field if it were even would
be a taurus and hour glass shape?" Then, during this pass: "Is there a line graph of the diminishing
returns of a water shield and the other forms of shielding?" (the chart below).

## Short answer, in plain words

- **How thick:** the spinning steel drum is about **17.5 cm** of steel. The water shell around it,
  which does not spin, is **10 m** of water, about **11.5 m** once it sits in sealed foam cells with
  room to freeze. With a gap of a few metres between them, the vacuum is roughly **14 to 17 m** below
  your feet. The shell's water weighs about **37 million tonnes**, nearly nine times the drum's steel.
- **A better material?** Not much better. Polyethylene does the job with about 7% less mass but is
  made of carbon, which the ship is short of. Liquid hydrogen is the best per kilogram but would be
  a 141 m deep tank of something that boils. Rock and slag are free mining waste and make a thinner
  layer (about 5 to 7 m) but need about 40% more mass. Water stays the right choice.
- **The real saving is mass the ship already carries.** Everything between a person and space
  counts, and in a drum that includes the floor under your feet and the floor across the drum above
  your head: steel, soil, buildings, and (his 2026-11-12 idea) stores and workshops placed as armour
  under the floor. That is roughly **2 to 3 t/m2 worth of water already there for free**, so the
  shell could probably shrink from 10 to about **7 to 8 t/m2**, saving about 7 to 11 million tonnes.
  This needs a proper radiation-transport calculation before the number changes.
- **Magnets work, but not for this.** A superconducting magnet bends charged particles away. It
  can stop solar storms completely, but a 10 m water shell already does that. Against cosmic rays,
  every study with the real structure modelled finds the field removes only about **a fifth to two
  thirds** of the dose, never the 99% the ship needs, because the most energetic particles go
  straight through and the magnet's own metal makes new radiation. Even Earth's whole magnetic
  field, far stronger than anything we could build around a drum, would save only about 2 t/m2 of
  the shield once the water is thick (chart 2 shows why: a field's help shrinks as the shield
  thickens). A drum-sized magnet would store as much energy as a few kilotons of explosive, need
  cryogenics over 3 km2 and could weigh as much as the water it saves.
- **His Earth picture is right**, and it explains the answer: Earth's field lines make nested
  doughnuts with funnel-shaped openings at the poles, where particles slide straight in. The
  arrangement studies favour around a cylinder is the toroid, which puts the field in a doughnut
  wrapped around the drum and leaves almost none where people live; its weak spots are the drum's
  two ends, like Earth's poles.
- **Recommendation:** keep the passive water shell; credit the ship's own floor and stores once a
  transport run confirms it (likely 7 to 8 t/m2 of shell); keep magnets for small craft (transports,
  expedition ships), where they halve the dose and remove the solar-storm risk at a mass a ship can
  carry.

![Dose against shielding mass, and what a magnetic field leaves](shield-curves.svg)

**The chart** ([shield-curves.svg](shield-curves.svg), drawn by [shield-curves.js](shield-curves.js)
from the cited points in [shield-curves.json](shield-curves.json); `node
docs/reference/research/2026-11-13-active-shielding/shield-curves.js` redraws it). Chart 1 is the
diminishing return he asked about: each extra tonne of water cuts the dose by a similar fraction (a
straight line on this log scale), so the last few tonnes buy little in mSv but are what take the
dose from 20 down to 5. Chart 2 shows what a magnetic field leaves of the dose at each depth. Every
marker is a value printed in a source (chart 2: the ratio of two values printed in the same source);
the dashed lines only join them; the dotted line past 7 t/m2 is the sizes research's extension, an
estimate, and it is where the decided 10 t/m2 sits. Liquid hydrogen and aluminium have no series:
no table for them at these depths was found in a source read this pass (section 2).

## 1. How thick it is today

The drum is Drum 1: radius 250 m, length 2,000 m, rim at about 50 m/s (data/ship/space_program.toml
`[drum]`); the shield is a non-rotating shell around it, so the drum carries no shield and the shell
carries no spin (master plan 1.5 item 8; the "unattached stationary shielding" design, `[scale]`).

| Layer, inside to out | Thickness | Mass per m2 | How it was found |
|---|---|---|---|
| Soil and buildings on the floor | about 1 m of soil where farmed | about 2.6 t/m2 | sizes research 6.2, judgement |
| Drum wall, steel S690QL | **17.5 cm** (14.0 cm for air alone; 8.7 cm at NASA's factors) | 1.37 t/m2 | sizes research 6.2, computed from F2, F17, F18 |
| Gap between the spinning rim and the still shell | a few metres | none | not yet sized (estimate) |
| Water shell, liquid | **10.0 m** | 10 t/m2 | 1 t/m2 = 1 m of water |
| ... as ice | 10.9 m | 10 t/m2 | ice is 0.917 g/cm3 (F34), computed |
| ... in sealed foam cells | **about 11.5 m** | 10 t/m2 plus the cell walls | estimate: walls take 5% of the volume and each cell keeps 9% free for freezing |

- **From the floor's underside to vacuum: about 14 to 17 m** (computed from the rows, with a 2 to 5 m
  gap as an estimate).
- **If the water rode on the spinning drum** instead, the steel would have to be **31 cm** (computed
  with the sizes research's formula, 200 MPa allowable, air plus 2.6 plus 10 t/m2). The non-spinning
  shell is why the drum stays at 17.5 cm.
- **Mass** (computed): the shell at a mean radius of about 261 m covers about 3.71 km2 of sides and
  ends, so 10 t/m2 is **about 37 million tonnes** of water (35.3 million on the drum's own 3.53 km2,
  sizes research section 7). The drum's cylinder of 17.5 cm steel is about **4.3 million tonnes**.
  At Drum 1's 6,000 to 7,800 people (master plan 3.4 and 3.5) the shell is about **4,700 to 6,200 t
  of water a person**.
- **What the 10 t/m2 buys:** the published table stops at 7 t/m2 of water (16.1 mSv a year, S1,
  F28). Extending its last step (x0.61 per tonne) gives 5 mSv at about **9.4 t/m2** and about 3.6
  mSv at 10 (computed, low confidence: the sizes research's F29 notes NASA's slab results flatten at
  depth). The decided 10 t/m2 is the 5 mSv target with a little margin on that extension.
- **Foam cells' walls are not wasted:** if the cell walls are a hydrogen-rich plastic they shield
  about as well per kilogram as water (S1: polyethylene is "a bit more effective than water"), so
  the walls' mass counts toward the 10 t/m2 (estimate).

## 2. Passive alternatives, per kilogram

**The materials.** For 5 mSv a year in deep space, from S1's Table 2 extended past its last row by
each column's own last step (computed, low confidence):

| Material | Mass for 5 mSv | Thickness | Notes | Source |
|---|---|---|---|---|
| Liquid hydrogen | the least per kilogram | **141 m** for 10 t/m2 (density 0.07099 g/cm3) | at 20 g/cm2 a Mars mission's dose was hydrogen 0.43 Sv, polyethylene 0.78, aluminium 0.87 (S2); MAARSS: past 225 days "the same mass of polyethylene or water allow more days for a mission than liquid hydrogen" because the tank's volume grows (S3). Cryogenic, boils off. Not for a drum | S2, S3; thickness computed |
| Polyethylene | about **8.7 t/m2** | about 9.2 m (0.941 g/cm3, S3) | 7 t/m2 gives 12.3 mSv against water's 16.1 (S1). It is 86% carbon (chemistry): about 32 million t of plastic, 28 million t of carbon, for Drum 1, and carbon is scarce aboard (sizes research section 5) | S1; computed |
| **Water (decided)** | about **9.4 t/m2** (10 decided) | 10 m | hydrogen and oxygen from icy asteroids; also the ship's reserve (below) | S1, F28; computed |
| Lunar regolith, standing in for rock and slag | about **13.4 t/m2** | 5.1 to 6.7 m (at 2.63 t/m3, basalt's density, F8; or 2.0 t/m3 rubble, `[scale]`) | per kilogram it needs 1.5 to 2.1 times water's mass for the same dose (computed from S1 Table 2 at 3 to 7 t/m2 of water). Free: mining waste | S1; computed |
| Steel, aluminium | the most | | no thick-shield table read; in NASA's slab tests aluminium's dose stops falling near 40 g/cm2 (F29). Metal is structure, not shield mass to add on purpose | F29 |

So water is within about 8% of the best practical material per kilogram, made of what the ship can
mine, and doubles as a store. Rock is the only alternative that saves anything (thickness, and
mining effort, since it is waste), at the cost of mass.

**What the ship already carries (dual use).** In a drum, every line of sight from a person to space
crosses either the floor below them or the floor across the drum above them, plus the air between;
only the lines through the end walls escape both. So the floor's mass shields everyone, the way the
operator's 2026-11-12 layering intended ("the hangar and other like utilitarian stuff is on the
outside to act as a layer of armor", master plan 1.3):

| What | Mass per m2 | Worth in water (estimate) | Note |
|---|---|---|---|
| Drum steel (17.5 cm) | 1.37 t/m2 | about 0.7 to 0.9 t/m2 | metal; counted at regolith's ratio, likely generous (estimate) |
| Soil and buildings | about 2.6 t/m2 (sizes research 6.2, judgement) | about 1.5 to 2 t/m2 | soil holds water; buildings hold plastics, water, people |
| Air straight across the drum | about 0.6 t/m2 (500 m at about 1.2 kg/m3, physics) | about 0.3 t/m2 | only for lines that cross the drum |
| A stores and workshop layer under the floor | whatever it weighs | about 0.5 to 1 per tonne | rides the spinning rim: each t/m2 adds 1.36 cm of steel, 0.107 t/m2 (computed, sizes research formula) |
| The ship's water reserve | 200 kg a person (J, NASA design figure) | 0.0004 t/m2 | about 1,560 t for 7,800 people: nothing at this scale (computed) |

- **The floor alone is worth roughly 2 to 3 t/m2 of water** (estimate), everywhere but the end
  walls. With it, the water shell for 5 mSv is probably **7 to 8 t/m2** instead of 10 (estimate),
  saving about **7 to 11 million tonnes** (computed at 3.71 km2).
- **A deliberate stores layer** under the floor (workshops, food stores, tanks) adds its own mass at
  about a tenth of that mass again in steel, so it is dual use too; how much the ship would keep
  there is a design choice, not yet made.
- **The other direction of dual use:** the shell's 37 million tonnes of water is the ship's reserve
  for life support, growth (the next drum's water), emergencies and reaction mass. Drawing some down
  thins the shield, so the reserve has a floor (estimate).
- **Layer order** (judgement, not sourced this pass): rock or slag outside, water inside, so the
  hydrogen-rich layer soaks up the neutrons the rock makes.
- **What would settle it:** one transport run of the real stack (water, gap, steel, soil, air, the
  far floor) in NASA's OLTARIS tool, which is free but needs an account (question 3).

## 3. Active shielding

### 3.1 His Earth picture: the torus and the hourglass

Earth's field is a dipole, like a bar magnet. Its field lines leave the planet near one pole, arc
out over the equator and come back in near the other, so they nest like layers of a doughnut (a
torus) around the planet. Charged particles caught on those lines spiral along them and bounce from
pole to pole, which is what the Van Allen belts are: "twin doughnut-shaped zones" with particles
"continually bouncing back and forth from pole to pole" (NASA, S5).

At the poles the lines pinch together and dive straight down: the waist of his hourglass. That
funnel is the weak spot. A particle coming straight down at a pole runs along the lines and is not
turned; at the equator it must cross them and is turned away: "the field at the poles is radial and
charged particles coming from the zenith direction can travel unaffected along the field lines. At
the equator, on the other hand, the field is orthogonal to the zenith direction and the shielding
effect is maximum" (Baldini 2014, S4). In numbers, a particle needs about 15 GV of rigidity (a
proton of about 14 GeV) to reach the equator from straight above, and "sub-GV values can be
reached moving close to the magnetic poles" (S4).

**Three ways to put a field around a drum** (the physics of each is standard; the studies that
chose them are named):

- **A dipole, like Earth's** (one big current loop around the drum's waist): the field runs
  straight through the drum along its axis, so people live in the strongest part, and its two
  funnels point out of the drum's ends. No study read here proposes it for a habitat.
- **A toroid** (coils wound around a doughnut-shaped tube that encloses the drum): the field lines
  run in circles around the drum, inside the tube's wall. Ideally there is no field in the hole,
  where people live, and none outside. A particle coming from the side must cross the circles and is
  bent along the drum, away from the people. The ends stay open, the toroid's version of Earth's
  poles. SR2S, the European study, chose it: "an isotropic protection around the habitat and a very
  low fringe field inside the internal module", but "Clearly a toroid surrounding the spacecraft
  leaves the endcaps of the habitat module free of the field (and therefore protection)" (S6).
  NASA's MAARSS study disagreed about real coil sets: "Most toroid configurations result in a large
  field within the habitat requiring complex shielding from the magnetic field for the crew and
  electronics", and chose instead seven solenoids (six around one) with a compensator coil (S7).
  Both views can be true: an ideal continuous toroid has no field inside; a real one made of
  separate coils with a structure leaks between them (judgement). For a drum 2 km long the open ends
  are 11% of the hull (computed), and the toroid is the configuration a study would start from.
- **Electric charge or plasma:** an electrostatic shield charges the habitat to repel protons. NASA's
  1975 study: "a 10-billion-volt potential would be required for even moderate shielding" (S11).
  A 2011 NASA concept needed 300 MV and "the ability to achieve 300 MV potential levels remains as
  future work"; its power "becomes increasingly more challenging as the spacecraft voltage is
  increased" (S13). The plasma "mini-magnetosphere" (S12) inflates a small dipole with captured
  plasma to deflect solar storms: a 3 m coil at a peak 6.4 T, 16 kW plus 5 kW for its cryoplant,
  about 1.5 t for a craft, and in its authors' words "An active deflector shield system would never
  be a replacement for passive shielding nor biological advances."

### 3.2 What a magnetic shield stops

- **Solar particle events** (mostly protons of tens to a few hundred MeV): yes. "Despite such
  limitations, a superconducting magnet could completely eliminate the risk due to SPE" (Musenich et
  al. 2018, S9). MAARSS: a 36 t coil array at 8 T m protected like 141 t of 75 cm polyethylene in a
  solar storm (S7). But the drum's shell already does this many times over: a storm shelter needs
  about 10 cm of water (F30), and the shell is 10 m.
- **Galactic cosmic rays: the lower half only.** A field turns back particles below a cutoff set by
  its bending power (field times depth, T m). By the simplest estimate, a proton turns back once
  bending power reaches its momentum per charge (computed; SR2S's own formula adds a factor up to 2
  for the angle, S6):

| Bending power | Protons turned back below | Nuclei (helium to iron) below | Example |
|---|---|---|---|
| 7.9 T m | about 1.6 GeV | about 0.6 GeV per nucleon | SR2S configurations A and B1 (S6) |
| 11.9 T m | about 2.8 GeV | about 1.1 GeV per nucleon | SR2S B2 (S6) |
| 20 T m | about 5 GeV | about 2.2 GeV per nucleon | where returns diminish (S8) |
| 50 T m | about 14 GeV | about 6.6 GeV per nucleon | Earth's equator, about 15 GV (S4) |
| 72 to 106 T m | 10 to 15 GeV per nucleon | | what SP-413 said 5 mSv would take (S11) |

  The cosmic-ray flux peaks "around 1 GeV for protons and 500 to 600 MeV/nucleons for ions" (SR2S,
  S10), so an SR2S-class field removes roughly the lower half of the spectrum. What is left, the
  energetic half, is what thick water is there to stop, and the magnet's own coils and structure
  break incoming particles into neutrons that no field can bend: "the secondary particles, produced
  by the interaction of cosmic rays with the active shielding materials, are the major source of the
  crew radiation dose" (S6).

### 3.3 The dose reductions actually reported

| Study | Shield | Reported | Source |
|---|---|---|---|
| NASA SP-413, 1977 | torus field, 0.5 GeV/n cutoff, 10 kt structure (0.5 t/m2) | "would actually increase the exposure to about 20 rem/yr"; a 10 to 15 GeV/n cutoff would reach 0.5 rem (5 mSv) but "the structural mass required to resist the magnetic forces between superconducting coils" "precludes this design" | S11 |
| MAARSS (NASA NIAC), 2012 | 6+1 solenoids at 8 T m, 6 m x 10 m habitat, about 40 t | 309 mSv a year (blood-forming organs) through the sides, 451 with open ends; "the reduction is not significant enough to provide significantly greater mission duration"; going from 1 to 2.5 T cut dose 25% for twice the mass | S7 |
| SR2S (EU), 2016 | toroid, 7.9 and 11.9 T m, 104 to 315 t | with the structure modelled, field on gives 55 to 59% of the free-space dose against 69 to 88% with it off; the field's own share "reaching about 20% for configuration B2"; about a factor of 2 overall, sides only | S6 |
| SR2S final report | | "a factor two in reduction of the CR dose with suitable technical developments" | S10 |
| Musenich et al., 2018 | toroids, limits analysis | limits from particles "higher than the shield cutoff energy" and secondaries; magnets not viable alone for 90% cuts (as summarised in S8) | S9 |
| Hesse-Withbroe and Arquilla, 2025 (preprint) | solenoids behind 12 g/cm2 of aluminium | 15 T m: 246 mSv a year; 100 T m: 188 (63% below the wall alone's 514); "greatly diminished performance" past 20 T m; "an order of magnitude or more are unlikely to be achieved using magnetic shields alone"; magnets beat passive mass only where a cut of more than about 60% is needed | S8 |
| Earth's own field (OLTARIS) | equatorial orbit, 600 km, plus polyethylene | 5 t/m2 gives 7.0 mSv a year against 35 in deep space; with 0.25 to 2 t/m2 at 500 km the dose never falls below 11.7 | S1 Tables 3, 4 |

Chart 2 puts these side by side. Even Earth's field, with the planet blocking about 30% of the sky
(computed at 600 km), leaves 7% of the deep-space dose behind 1 t/m2 but 20% behind 5 t/m2: **the
thicker the passive shield, the less any field adds**, because the shield has already removed the
particles the field would have bent. The magnets that have been designed for craft leave 37 to 79%
of the dose even behind thin walls.

### 3.4 A magnet the size of Drum 1 (computed)

A toroid around the drum's 2 km barrel, its field between radius 255 m and 255 m plus the field
region's depth (ideal toroid, field falling as 1/r, SR2S's formula in S6; ends not included):

| Bending power | Field region | Field at its inner edge | Stored energy | As TNT | Magnet mass at 1.3 to 21 MJ a tonne |
|---|---|---|---|---|---|
| 10 T m | 50 m | 0.22 T | 2.8 TJ | 0.67 kt | 0.13 to 2.1 million t |
| 10 T m | 10 m | 1.0 T | 13 TJ | 3.1 kt | 0.6 to 10 million t |
| 20 T m | 50 m | 0.44 T | 11 TJ | 2.7 kt | 0.5 to 8.6 million t |
| 50 T m (Earth-like) | 50 m | 1.1 T | 70 TJ | 17 kt | 3.3 to 54 million t |
| 100 T m | 50 m | 2.2 T | 280 TJ | 67 kt | 13 to 215 million t |

- **Mass per stored energy** spans the magnets that exist: the ATLAS barrel toroid, the largest
  superconducting magnet system yet built, stores 1,080 MJ and weighs 830 t (1.3 MJ a tonne, S14);
  ITER's magnets store 51 GJ in 10,000 t (5.1, S15); SR2S's lightweight space design, by its
  published size, about 21 MJ a tonne (computed from S6's Table 1). A deeper field region at a
  weaker field stores less energy for the same bending power, which is why 50 m beats 10 m.
- **For scale:** ITER's whole magnet system is 51 GJ (S15) and CMS's solenoid 2.7 GJ (S14). The
  smallest drum magnet above is about 55 ITERs.
- **Field for people and electronics:** ICNIRP's limits for static fields are 400 mT for the public
  ("acute exposure of the general public should not exceed 400 mT"), 2 T for workers' heads and
  trunks (8 T in controlled work), and for anyone with a pacemaker or other implant "much lower
  restriction levels, such as 0.5 mT" (S16). So the field region (0.2 to 2 T above) is a no-living
  zone, crews working there stay under 2 T, and homes need under 0.5 mT, which is why MAARSS added a
  compensator coil to cancel the leak into the cabin (S7). The same 0.5 mT is a sensible ceiling for
  ordinary electronics (judgement; no source read for electronics). A toroid's field runs along the
  rim's direction of motion, so the spin itself induces no voltage, but any stray field crossing a
  50 m/s steel rim would make eddy currents and drag (physics, not sourced): one more reason the
  field must stay out of the drum.
- **Power and cold:** a superconducting magnet in persistent mode needs no power to keep its current
  (SR2S runs at 10 K on cryocoolers, S6); the power goes to keeping it cold. MAARSS's craft shield
  needed about 2 kW for its coils' flux pumps and 26 kW for its compensator coil (S7). Scaled
  naively by area to Drum 1's 3.14 km2 that is about **0.5 GW**, more than five times the drum's whole 84 MW
  (master plan 1.5 item 6): an upper guide only (estimate, low confidence), since a huge cryostat
  leaks less per square metre and the shadowed side of a ship radiates heat to space for free.
- **What a quench means:** a quench is the moment part of the coil stops superconducting; the
  current then meets resistance and the stored energy turns to heat. "If no precautions have been
  taken, when a magnet quenches the entire stored energy will be dissipated in the area of the quench
  initiation point" (S14). MAARSS: one 400 MJ coil "is sufficient to melt about 570 kg of copper",
  and "An uncontrolled release of this energy into the coil itself would cause significant damage to
  the magnet with high probability" (S7). At that rate the drum magnets above hold enough to melt
  **4,000 to 400,000 t of copper** (computed). High-temperature superconductors are far harder to
  tip into a quench (about 10,000 times the energy, S7) but spread one slowly ("about 2 cm/sec",
  S7), which makes it harder to catch. Any real design would be many separate coils with their own
  dump resistors, so one fault cannot empty the whole ring (judgement).

## 4. A combined design: magnet plus a thinner shell

What each option would need for 5 mSv a year (water for Drum 1's shell at 3.71 km2):

| Option | Water shell | Water mass | What else | Verdict |
|---|---|---|---|---|
| A. Water only (decided) | 10 t/m2 | 37 million t | nothing | works; the baseline |
| B. Water, crediting the floor and stores | about 7 to 8 t/m2 (estimate) | 26 to 30 million t | a transport calculation to prove it | **saves 7 to 11 million t for nothing** |
| C. B plus an SR2S-class toroid, 10 to 20 T m | 0.5 to 1.4 t/m2 thinner than B (estimate) | saves 1.9 to 5.2 million t more | a 3 to 11 TJ magnet of 0.1 to 9 million t, cryogenics over 3 km2, a quench hazard of up to a few kilotons | gains a few million tonnes at best, loses several at worst: not worth it |
| D. B plus an Earth-strength toroid, 50 T m | 1.5 to 2.6 t/m2 thinner than B (estimate) | saves 5.6 to 9.6 million t more | a 70 TJ (17 kt) magnet of 3 to 54 million t | likely a loss, and nothing near it has been built |

How C and D were estimated (estimate, low confidence): a field that cuts the dose at depth by a
share f saves about ln(1 - f) / ln(0.61) t/m2 of water, since each tonne of water cuts the dose to
about 0.61 of itself (S1's last rows). SR2S's field cut 21 to 28% behind thin walls (chart 2) and
any field's help shrinks with depth, so f is at most about 0.2 to 0.5 behind 7 t/m2 (0.45 to 1.4
t/m2). For D, S1's own Earth-orbit numbers: polyethylene under Earth's field reaches 5 mSv at about
6.1 t/m2 (7.2 if Earth's shadow is taken out) against about 8.7 in deep space (computed by extending
each column), a saving of 1.5 to 2.6 t/m2.

**The trade in one line:** the magnet trades millions of tonnes of plain water, which needs no
power, cannot fail all at once and doubles as the ship's reserve, for a machine that may weigh as
much as it saves, needs cryogenics on the scale of the ship's whole power plant and stores kilotons
of energy that a fault can release. Water wins for the drum.

## 5. Recommendation

1. **Keep the passive water shell as the drum's shield** (decided, 1.5 item 8). Do not add a magnet
   to the drum.
2. **Credit what the ship already carries.** Count the floor (steel, soil, buildings), the air
   across the drum and any stores layer under the floor; run the real stack through a transport
   code. Expect about 7 to 8 t/m2 of shell for 5 mSv, about 26 to 30 million t of water instead of
   37. Keep 10 t/m2 in data/ship/space_program.toml until that run is done.
3. **Use mining slag outside the water when the fleet has it**: it is free, makes a thinner layer,
   and needs about 1.5 to 2 times water's mass for the same dose.
4. **Magnets belong on small craft**, where 10 m of water is impossible and trips are short: an
   SR2S-class magnet halves the cosmic-ray dose (S10) and removes the solar-storm risk (S9) for a
   few tens of tonnes (S7, S10). A proposal for the game: an active shield as an upgrade for
   transports and expedition ships, with its power draw, cooling and quench failure mode simulated,
   and a plain toggle per the dual-mode rule.
5. **No finding is adopted yet**: the shield decision stands, so nothing changes in data. The
   negative result (magnets do not replace the drum's water) is written down as a dated finding so
   the question is not argued again from scratch:
   [findings/2026-11-13-active-shielding.md](../../findings/2026-11-13-active-shielding.md).

**What stays uncertain:**

- Everything past 7 t/m2 of water is an extension of one table (S1). NASA's slab results flatten at
  depth (F29); a different code or geometry could move the 5 mSv point by a tonne or two.
- The floor's water-equivalent uses regolith's ratio for steel and soil; steel is probably worse.
- Chart 2's Earth series includes the planet's shadow (about 30% of the sky), so it flatters the
  field somewhat.
- SR2S counted only particles from the sides; MAARSS and Hesse-Withbroe used other configurations
  and metrics; Hesse-Withbroe is a preprint.
- The drum magnet's mass and cryogenic power are scaled from machines a hundredth of its size.
- The solar-minimum year (1977 to 1978) is a near-worst case. Over the Sun's cycle the dose varies
  by "on the order of a factor of 2" (MAARSS, S7) to "about a factor of three" (S1, citing Robbins
  1996), so 10 t/m2 is conservative most years.

## Questions for the operator

1. **Drop magnetic shielding from the drum's plan?** Recommendation: yes; keep magnets as an option
   for small craft.
2. **Count the floor and stores toward the 5 mSv, thinning the shell to about 7 to 8 t/m2 once a
   transport calculation confirms it?** Recommendation: yes, and keep 10 t/m2 in the data until then.
3. **May an agent register an OLTARIS account** (NASA's free web tool; registration needed) to run
   the real layered stack, or would you rather register it yourself? Recommendation: yes, either way;
   it settles questions 2 and the past-7-t/m2 extension in one sitting.
4. **A slag layer from mining waste outside the water**, once the fleet mines? Recommendation: yes,
   as a later rung; water stays the base.
5. **An active-shield upgrade for transports and expedition craft in the game?** Recommendation: yes,
   a later rung, with power, cooling and quench modelled.

## Files in this folder

- [sources.md](sources.md): every source, its date, the quotations, and what could not be read.
- [shield-curves.json](shield-curves.json): the cited points behind the chart.
- [shield-curves.js](shield-curves.js): draws the chart; [shield-curves.svg](shield-curves.svg): the
  chart (follows the viewer's light or dark setting).
