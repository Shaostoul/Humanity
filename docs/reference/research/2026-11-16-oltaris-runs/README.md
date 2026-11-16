# OLTARIS runs for the ship's radiation shield: the plan (2026-11-16)

The operator's account on NASA's OLTARIS (On-Line Tool for the Assessment of Radiation In Space) was
approved on 2026-11-16. This note lists the runs that settle the shield questions the design documents
leave open, so they can be done in one sitting. The account is the operator's: he signs in himself;
results are cited with credit to NASA OLTARIS in the design documents, as his registration promised.
Before anything is automated on the site, its own terms of use are read and followed.

## The questions the runs settle

1. **The shield's areal density for 5 mSv a year** (docs/design/mothership-master-plan.md, "The
   shield"): the design holds about 10 t/m2 of water in valved compartments, from published OLTARIS
   tables (docs/design/closed-loop-test.md: 16.1 mSv a year behind 7 t/m2 of water in deep space;
   docs/reference/findings/2026-11-07-drum-limits.md).
2. **Whether the floor, soil and stores count toward it**: decided in principle ("Makes sense to
   include it"); the data stays at 10 t/m2 of water until a transport calculation confirms it.
3. **Slag behind the water** (docs/design/slag.md, "Radiation"): each t/m2 of slag is estimated to
   multiply the dose by 0.65 to 0.79; "one OLTARIS run of the real stack would settle it".

## Units: the range to run

1 t/m2 = 100 g/cm2, so the design's 10 t/m2 of water is **1,000 g/cm2**. The registration's
purpose text says "5 to 20 g/cm2": that range is 50 to 200 times too thin to reach 5 mSv a year in
deep space and would only show doses far above it. The runs need to span about **100 to 1,500
g/cm2** (1 to 15 t/m2). First check, in OLTARIS's own documentation, the deepest thickness its
slab and sphere tools accept; if it stops short of 1,500 g/cm2, run to its limit and note the
extrapolation as an estimate, never as a result.

## The runs

Environment for each: galactic cosmic rays at solar minimum (the worst case for GCR), free space
(no planet's shadow or field), and separately the design solar particle event OLTARIS offers (for
example the August 1972 or the October 1989 event spectrum); quantity: effective dose (and dose
equivalent) for the reference human, in mSv a year for GCR and mSv per event for the SPE.

| Set | Shield, inside to outside | Thicknesses | Settles |
|---|---|---|---|
| A | Water alone | 100, 250, 500, 750, 1,000, 1,250, 1,500 g/cm2 | Question 1: the dose curve, and where it crosses 5 mSv a year |
| B | Water at the thickness A finds, then slag (or regolith as its stand-in, if slag is not a material choice) | slag 0, 100, 200, 300 g/cm2 | Question 3: slag's real factor against the 0.65 to 0.79 estimate |
| C | The habitat's own mass inside the water: floor and soil (regolith as soil's stand-in), stores (water or polyethylene as a stand-in for food) | soil 50 and 100 g/cm2, stores 10 and 30 g/cm2, with water reduced to keep the total | Question 2: how much of the 10 t/m2 the floor, soil and stores can replace |
| D | Equal mass of water, polyethylene, aluminium and regolith | 250, 500, 1,000 g/cm2 | The material comparison the Library's teaching material can quote |

Record for each run: the inputs exactly as entered, the date, the OLTARIS version shown, and the
result file. Store the numbers (not OLTARIS's files, unless its terms allow sharing them) in a dated
finding, docs/reference/findings/<date>-oltaris-shield.md, and correct the design documents' figures
from it.
