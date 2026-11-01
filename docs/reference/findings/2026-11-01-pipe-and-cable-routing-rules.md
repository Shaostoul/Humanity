# How must pipes, hoses and cables be bent, supported, separated, sloped and passed through walls?

**Research date: 1 November 2026.** Everything below was read on that day. If you are reading this
later, check each source's own date against anything that has changed since: the model codes
(the International Plumbing Code, the Uniform Plumbing Code, the National Electrical Code) publish a
new edition every three years, US federal regulations are amended without notice to anyone reading
an old copy, and makers revise their data sheets (one read here was revised two days before the
research day).

This is a reading of public sources by an AI working on the project, not legal or engineering
advice. Nobody who wrote it is a lawyer, a licensed electrician, a plumber, a marine engineer or a
building official. The people who decide how these rules apply to a real installation are the
authority having jurisdiction (a building or electrical inspector, a classification society or the
US Coast Guard for a vessel), working from the edition their jurisdiction has adopted.

Companion finding, same day: [2026-11-01-manual-door-releases.md](2026-11-01-manual-door-releases.md)
(how a powered door must open by hand). The design these numbers feed:
[docs/design/spline-routing.md](../../design/spline-routing.md).

## The question

For the runs a person lays in the game (copper and plastic pipe, hose, gas tubing, power and data
cable, conduit), what do primary sources say about the tightest bend, the spacing of supports, the
distance kept between power, data and fluid lines, the slope of a drain, and how a run passes
through a wall, a deck or a stud, so that the game's "to code" rules can be data rows with a
source each?

## How quotations are handled here

The findings README asks for a quotation in every finding. Two kinds of source were read:

- **US government text** (the Code of Federal Regulations, the Unified Facilities Guide
  Specifications, FAA and NASA documents) is not under copyright, so each such finding quotes the
  words that carry the rule, exactly, under 15 words.
- **Copyrighted text** (makers' manuals and data sheets, the model codes published by the ICC and
  IAPMO, IEEE and IEC standards) is not quoted, apart from ONE short quotation in finding 3. For
  these, the finding gives the numbers (facts, not expression), the table or page they sit in and
  the URL, so a later reader can open the same page and read the same words. Where a number was
  transcribed from a web table whose text extraction ran cells together, the finding says so.

Downloaded copies of every source are in the research session's scratch folder; they are not
committed (most are copyrighted).

## Short answer

The numbers, by rule. "Ours" in the last column means the design must still choose; every other
row has a primary source below.

| Rule | What the sources say | Findings | Becomes a data row |
|---|---|---|---|
| Copper tube bend radius | Annealed (soft) tube: 3/4 in (19 mm) at 1/4 in nominal up to 9 in (229 mm) at 1 1/4 in; drawn (hard) tube may be bent too, with the right bender, 1 3/4 in (44 mm) at 3/8 in to 3 in (76 mm) at 3/4 in; no drawn row above 3/4 in | 1 | yes, per size and temper |
| PEX bend radius | At least 6 times the outside diameter (3.8 in, 97 mm, for 1/2 in); 3 times that against the coil for some PEX | 3 | yes, as a multiple of OD |
| Rigid PVC and CPVC | Change direction with fittings; heat bending may damage the pipe; do not heat-bend CPVC | 4 | yes: bends not allowed, fittings only |
| Steel pipe bends (ships) | Any hot or cold method, to any radius, provided the bend meets ASME B31.1's conditions; no fixed number | 5 | yes, the radius left to the bender row |
| Hose bend radius | Maker data by size: 2 in (50 mm) at 1/4 in bore to 8 1/4 in (210 mm) at 1 1/4 in, for one hydraulic hose | 6 | yes, per hose row (ours where no maker row) |
| Gas tubing (CSST) bend radius | 9/16 in at 3/8 in up to 4 in at 2 in, one maker | 7 | yes |
| Power cable bend radius | NM-B and THHN data sheets list radii about 4 times the overall diameter (our arithmetic); the NEC's own multiple was not read; ships 4D to 7D by construction; aircraft bundles 10 times the largest wire | 19, 22, 23, 42 | yes, per cable row |
| Data cable bend radius | UTP at least 4 times the diameter; fibre 20 times while pulling, 10 times installed | 20, 21 | yes |
| Hand-bent EMT conduit | Bender centreline radius 4 5/8 in (117 mm) for 1/2 in, 5 1/2 in (140 mm) for 3/4 in, 7 3/8 in (187 mm) for 1 in; marked angles 10, 22.5, 30, 45, 60 degrees | 24 | yes, per bender row |
| How rigid runs change direction | "symmetrical bends or cast-metal fittings"; exposed conduit runs parallel with or at right angles to walls and ceilings | 26 | yes (the rigid behaviour class) |
| Support spacing, copper | 6 to 12 ft by size and source; the sources disagree (table in finding 9) | 2, 9, 11, 12 | yes, per profile |
| Support spacing, plastic | Plastic water pipe 3 ft (0.91 m) and plastic drain 4 ft (1.22 m) in a manufactured home; PEX 32 in (0.81 m) up to 1 in, 48 in (1.22 m) above | 3, 8 | yes |
| Support spacing, steel | 8 ft at 1 in and under, up to 20 ft at 8 in and over (hanger rods 3/8 to 1 1/4 in) | 9 | yes |
| Supports near bends and valves | Within 1 ft (0.3 m) of each change in direction; 5 ft (1.5 m) apart at valves; within 3 ft of the end of a runout | 9, 10 | yes |
| Cable supports | NM cable every 4 1/2 ft (1.37 m) and within 12 in (0.3 m) of each box (manufactured home); ships every 24 in (610 mm) on small passenger vessels, 48 in on towing vessels; aircraft clamps 24 in; spacecraft harness ties 19 to 76 mm by bundle size | 13 to 17 | yes, per profile |
| Data cable supports | J-hooks 4 to 5 ft (1.2 to 1.5 m) | 18 | yes |
| Communications next to power | At least 2 in (50.8 mm) indoors (OSHA); 12 in (300 mm) when parallel (a DoD specification) | 27, 20 | yes, both, per profile |
| Above electrical equipment | No foreign piping in the space 6.0 ft (1.83 m) above a panel; ships: piping not run over switchboards, which carry drip shields | 28, 29 | yes |
| Pipe and wire in one hole | Not allowed (manufactured homes) | 30 | yes |
| Shared supports | Electrical raceways never share supports with piping or ducts (a DoD specification) | 31 | yes |
| Hot pipes | Conduit 6 in (150 mm) from flues and steam or hot-water pipes; PEX 6 in from a gas or oil water-heater flue | 31, 34 | yes |
| Wiring and flammable fluid lines | 6 in or more whenever possible, 2 in minimum, wiring above the fluid line (aircraft) | 32 | yes, for the ship and spacecraft profiles |
| Cable to water pipe, generally | No general distance found in any source read | 35 | no number; the design says why |
| Drain slope | 1/4 in per foot (2.1%); 1/8 in per foot (1.0%) where that is impractical (with a cleanout, or for larger pipe) | 36, 37 | yes |
| Drain changes of direction | Which fitting may turn which way (quarter bends, sweeps, wyes, sanitary tees) | 38 | yes |
| Through walls and decks | Penetrations keep the bulkhead's integrity, no heat-sensitive metal where fire would defeat it; fire stops every 7 m (21.5 ft) vertically for some cable; watertight glands or transits | 39, 40 | yes, per wall rating |
| Through studs | Steel plates where a pipe or cable passes within 1 1/2 in of a stud's face | 41 | yes |
| The NEC, TIA-568/569, SAE J517, MSS SP-58, ASME B31 | Not read: account-only or sold | 42 to 44 | no |

**What this does not settle:** the National Electrical Code's own numbers (bend radius of NM cable,
the 360-degree limit between pull points, the conduit bend table, the communications separation in
its current numbering), the current (2024) editions of the plumbing codes apart from the PEX row a
PEX guide reprints, and maker load ratings for cable protector ramps. "What is still unknown"
below says what would settle each.

## Findings

### Bending pipe, tube and hose

**1. Copper tube bend radius (Copper Development Association, The Copper Tube Handbook, web
version).** Source: Table 4.1, https://copper.org/resource-library/copper-tube-handbook-table-4-1/,
and the Bending chapter,
https://copper.org/markets-and-applications/building-construction/strengthening-plumbing-systems/copper-tube-handbook/copper-tube-handbook-web-version/4-bending/.
Date: no edition shown; the pages carry "Copyright © 2026". Copyrighted: numbers only. Table 4.1,
minimum bend radius by nominal size, type and temper (inches):

| Nominal size | Types | Annealed | Drawn |
|---|---|---|---|
| 1/4 | K, L | 3/4 | (no row) |
| 3/8 | K, L (drawn: K, L, M) | 1 1/2 | 1 3/4 |
| 1/2 | K, L (drawn: K, L, M) | 2 1/4 | 2 1/2 |
| 3/4 | K, L | 3 | 3 |
| 1 | K, L | 4 | (no row) |
| 1 1/4 | K, L | 9 | (no row) |

The chapter says both annealed and drawn tube may be bent with the correct hand bender; the lever
bender in its figure 4.1 is for annealed tube only. The asterisk on the table's "Radius" heading
has no footnote in the HTML (not read). So hard copper larger than 3/4 in has no listed bend: it
changes direction with fittings.

**2. Copper tube supports (the same handbook, "General Considerations").** Source:
https://copper.org/markets-and-applications/building-construction/strengthening-plumbing-systems/copper-tube-handbook/copper-tube-handbook-web-version/3-design-and-installation-data/general-considerations/.
Date: as finding 1. Numbers only: drawn tube horizontally about every 8 ft (2.44 m) at 1 in and
smaller and about every 10 ft (3.05 m) larger, unless the plumbing code says otherwise; vertically
every storey or about 10 ft; annealed tube at least every 10 ft vertically and 8 ft horizontally.
The page also gives copper's expansion, 0.0000094 in per in per degree F (100 ft heated 100 F grows
1.128 in), and an expansion-loop length formula.

**3. PEX bend radius and supports (Plastics Pipe Institute and Plastic Pipe and Fittings
Association, PEX Plumbing Distribution Systems Design and Installation Guide, Third Edition).**
Source, chapter 9:
https://www.plasticpipe.org/common/Uploaded%20files/Technical/PEX%20Plumbing%20Design%20Installation%20Guide%20Third%20Edition_Ch9.pdf
(the whole guide is over 10 MB and was read by chapter). Date: "April 2026"; first published 2006,
revised 2013 and 2026; prepared by Home Innovation Research Labs. Quotation (the one copyrighted
quotation in this document, the doubled "than" is the source's): the free bend radius is "not
less (i.e., no tighter than) than six times the actual outside diameter", unless the PEX maker says
otherwise. For certain PEX, bending against the coil's direction needs three times that. Table 9.1
(p. 98; the extraction shifted its columns, so each row was re-paired by six times the OD, which the
text's own 1/2 in example of 3.8 in confirms): 3/8 in 3.0 in; 1/2 in 3.8 in; 3/4 in 5.3 in; 1 in
6.8 in; 1 1/4 in 8.3 in; 1 1/2 in 9.8 in; 2 in 12.8 in; 2 1/2 in 15.8 in; 3 in 18.8 in; 4 in 24.8 in.
Snap-on bend supports let a 90-degree sweep replace an elbow. Supports: horizontal 32 in (0.81 m)
up to 1 in and 48 in (1.22 m) from 1 1/4 in; vertical at each storey and mid-storey, 60 in at most;
in a support channel 6, 8 and 10 ft by size. Holes through wood 1/4 in larger than the tube. Keep
PEX at or below 180 F, 6 in from a gas or oil water heater's flue (18 in metal connectors).
Chapter 4 reprints, "with permission of the publishers", the 2024 IRC's Table P2605.1 (PEX 32 in
horizontal, 10 ft vertical) and the 2024 UPC's 312.9 (a No. 18 gauge steel plate where plastic or
copper pipe is within 1 in of a framing member's edge).

**4. Rigid PVC and CPVC (Charlotte Pipe, Plastics Technical and Installation Manual, TM-PL).**
Source (the maker's manual on a distributor's server):
https://webassets.hajoca.com/ownerguides/611942/61194212498.pdf. Date: "Updated July 7, 2021"; a
newer edition may exist. Numbers only. Page 52: an external heat source used to bend ABS, CPVC or
PVC may damage the pipe and fittings, so change direction with fittings; the CPVC section says not
to heat-bend CPVC. Page 47: horizontal support spacing for PVC Schedule 40 by temperature, 1/2 in
4.5 ft at 60 F falling to 2.5 ft at 140 F, 2 in 6 ft to 3 ft (the 4 in row extracted badly and is
not used); the page notes that most codes cap ABS and PVC at 4 ft for every size, CPVC at 3 ft up
to 1 in and 4 ft above. Page 49: the maker's coiled CPVC (CTS) bends to 18 in at 1/2 and 3/4 in and
24 in at 1 in.

**5. Steel pipe aboard a ship (46 CFR 56.80-5 and 56.80-15).** Source: the eCFR API, Title 46, Part
56, current as of 2026-10-28,
https://www.ecfr.gov/api/versioner/v1/full/2026-10-28/title-46.xml?part=56. Quotation, 56.80-5:
"Pipe may be bent by any hot or cold method and to any radius", provided the bend is free of cracks,
substantially free of buckles, and meets ASME B31.1's 102.4.5 and 104.2.1. 56.80-15(c) asks stress
relief after cold bending for carbon steel 3/4 in thick or more and for some alloy pipe. So the
federal rule sets no radius: the bender (and B31.1, not read) does.

**6. Hydraulic hose bend radius (Parker Hannifin, Catalog 4400, hose section, 451TC hose).** Source
(the maker's catalog on a distributor's server):
https://www.wilson-company.com/images/document/HPD/Section_A_Hose.pdf, page A-10; the definition of
the radius from Parker's Tech Bulletin PIB-06.005.1 (dated 3/28/06),
https://www.crossco.com/wp-content/uploads/2019/08/Minimum-Bend-Radius.pdf. Date: the catalog PDF's
file date is 2015-01-13, so it is probably superseded. Numbers only: minimum bend radius by dash
size, -4 (1/4 in bore) 2 in (50 mm), -6 2 1/2 in (65 mm), -8 (1/2 in) 3 1/2 in (90 mm), -10 4 in
(100 mm), -12 4 3/4 in (120 mm), -16 (1 in) 6 in (150 mm), -20 8 1/4 in (210 mm); rated as half the
SAE 100R1 radius. The radius is the tightest arc before the hose's life is greatly reduced, measured
to the inside of the bend; tighter risks kinking. A garden or water hose was not researched (see
"What is still unknown").

**7. Corrugated stainless steel gas tubing (Omega Flex, TracPipe CounterStrike Design and
Installation Guide, FGP-001 12/16).** Source (the maker's guide on a third-party host):
https://inspectapedia.com/plumbing/Flexible-Gas-Pipe-Installatin-TracPipe-2016.pdf. Date: "12/16";
a newer FGP-001US 04/22 exists and was not read. Numbers only: Table 4-1, minimum bend radius 3/8 in
tubing 9/16 in, 1/2 in 3/4 in, 3/4 in 1 in, 1 to 1 1/2 in 3 in, 2 in 4 in (several tight bends add
pressure drop); Table 4-2, horizontal supports 3/8 in 4 ft, 1/2 in 6 ft, 3/4 to 2 in 8 ft; vertical
10 ft at most.

### Supports

**8. Plastic pipe in a manufactured home (24 CFR 3280.608).** Source: the eCFR API, Title 24, Part
3280, Subpart G, current as of 2026-10-28,
https://www.ecfr.gov/api/versioner/v1/full/2026-10-28/title-24.xml?part=3280&subpart=G; the section
last amended at 86 FR 2521, Jan. 12, 2021. Quotation, (b): "horizontal plastic drainage piping must
be supported at intervals not to exceed 4 feet". Plastic water piping every 3 ft; vertical piping at
each storey; unless the incorporated standards or the pipe's maker say otherwise. (a) provide for
expansion and contraction; (c)(3) hangers for plastic pipe must not compress, distort, cut or abrade
it, and must let it move.

**9. Hanger spacing for steel and copper (US DoD, Unified Facilities Guide Specification UFGS 23 05
15).** Source:
https://nibs-s3-wbdg3-production.s3.us-east-1.amazonaws.com/FFC/DOD/UFGS/UFGS%2023%2005%2015.pdf
(wbdg.org redirects there). Date: "UFGS-23 05 15 (May 2022)", Change 2, 08/24. A government contract
specification, not law. Quotation: the table gives the "following minimum rod size and maximum
allowable hanger spacing for" each pipe size. In inches of pipe, rod size, then steel and copper
spacing in feet: 1 and smaller 3/8, 8, 6; 1 1/4 to 1 1/2 3/8, 10, 8; 2 3/8, 10, 8; 2 1/2 to 3 1/2
1/2, 12, 12; 4 to 5 5/8, 16, 14; 6 3/4, 16, 16; 8 to 12 7/8, 20, 20; 14 to 18 1, 20, 20; 20 and over
1 1/4, 20, 20. Supports no more than 3 ft from the end of each runout and 1 ft from each change in
direction; shorten the span in proportion at valves. The specification's metric table gives DN50
copper 3050 mm (10 ft) where its inch table gives 8 ft: an internal inconsistency.

Copper support spacing as four sources give it (horizontal; the sources disagree):

| Source | Small sizes | Larger sizes |
|---|---|---|
| CDA handbook (finding 2) | about 8 ft (1 in and under) | about 10 ft |
| IPC 2009 (finding 11) | 6 ft (tubing 1 1/4 in and under) | 10 ft (1 1/2 in and over); copper pipe 12 ft |
| UPC 2009 (finding 12) | 6 ft (1 1/2 in and under) | 10 ft (2 in and over) |
| UFGS 23 05 15 (this finding) | 6 ft (1 in and under) | 8 ft, then 12 ft and more from 2 1/2 in |

**10. Supports at valves and bends (UFGS 22 00 00, Plumbing, General Purpose).** Source: the same
server, UFGS%2022%2000%2000.pdf. Date: "(May 2024)", Change 1, 11/24. Quotation: "supports must be
spaced not over 1.5 m 5 feet apart at valves". Supports within 1 ft of the joint at each change in
direction; spacing otherwise per MSS SP-58 (not read, finding 44); plastic pipe's spacing chosen
for 120 F (PVC) and 180 F (CPVC); vertical supports at each floor, at most 15 ft apart and 8 ft
from a riser's ends.

**11. Hanger spacing and protection in a model plumbing code (International Plumbing Code, 2009
edition, as posted by Public.Resource.Org).** Source:
https://archive.org/download/gov.law.icc.ipc.2009/icc.ipc.2009.html. Date: the 2009 edition,
superseded by 2012, 2015, 2018, 2021 and 2024. Copyrighted by the ICC: numbers only (the OCR reads
"114" for 1/4). Table 308.5, horizontal then vertical spacing in feet: ABS 4, 10; copper pipe 12,
10; copper tubing 1 1/4 in and smaller 6, 10; 1 1/2 in and larger 10, 10; PEX 2.67 (32 in), 10;
CPVC 1 in and smaller 3, 10, larger 4, 10; steel 12, 15; PVC 4, 10; cast iron 5, 15 (10 ft with 10 ft
lengths); mid-storey guides for 2 in and smaller. 308.6: sway bracing at changes of direction over
45 degrees for pipe 4 in and larger. 305.8: a 0.0575 in (No. 16 gauge) shield plate where pipe is
within 1 1/2 in of a stud's or joist's edge.

**12. The Uniform Plumbing Code, 2009 edition (Public.Resource.Org's OCR).** Source:
https://archive.org/download/gov.law.iapmo.upc.2009/iapmo.upc.2009_djvu.txt. Date: 2009, superseded.
Copyrighted by IAPMO: numbers only. Table 3-2 (section 314.0): copper 6 ft at 1 1/2 in and smaller
and 10 ft at 2 in and larger horizontally, each floor and 10 ft at most vertically; steel and brass
10 ft at 3/4 in and smaller, 12 ft at 1 in and larger, every other floor and 25 ft vertically; steel
gas pipe 6, 8 and 10 ft by size; Schedule 40 PVC and ABS drain 4 ft at every size, with expansion
every 30 ft; CPVC 3 ft to 1 in, 4 ft above.

**13. Cable supports in a manufactured home (24 CFR 3280.808, 3280.804).** Source: the eCFR API,
Title 24, Part 3280, Subpart I, current as of 2026-10-29,
https://www.ecfr.gov/api/versioner/v1/full/2026-10-29/title-24.xml?part=3280&subpart=I; 3280.808
last amended at 89 FR 75757, Sept. 16, 2024. Quotation, 3280.808(d): nonmetallic sheathed cable
"secured in place at intervals not exceeding 4½ feet and within 12 inches" of every cabinet, box or
fitting (the eCFR's XML sets the half as a fraction). (c): exposed cable 15 in or less above the
floor is protected; (j): 4 in of free conductor at each box; 3280.804(f): a clear working space at
least 30 in wide and 30 in in front of the panelboard.

**14. Cable supports aboard ships (46 CFR 120.340, 143.575; IEEE 45.8-2016; IEC 60092-352).**
Sources: the eCFR API, Title 46, current as of 2026-10-29 (Subchapter J and the vessel parts);
IEEE 45.8-2016 as posted by Public.Resource.Org,
https://law.resource.org/pub/us/cfr/ibr/inc/ieee/ieee.45.8.2016.pdf (approved 29 January 2016,
incorporated by reference in 46 CFR 111.60-5); IEC 60092-352 third edition (2005-09), as posted at
https://law.resource.org/pub/us/cfr/ibr/inc/iec/iec.60092-352.1979.pdf (the file name says 1979; the
document inside is the 2005 edition). Quotation, 46 CFR 120.340(b)(4), small passenger vessels:
"metal supports spaced not more than 610 millimeters (24 inches) apart". 120.340(b)(5) also says
cable must not be installed with sharp bends, with no number. 46 CFR 143.575(a)(2), towing vessels:
supports and retention every 48 in, and 24 in clear of movable machinery. 46 CFR 111.60-5(a) itself
gives no radius or spacing; it points to IEEE 45.8 section 6 or IEC 60092-352. IEEE 45.8 (copyrighted,
numbers only): supports no farther apart than the frames or 610 mm (24 in), retention every 610 mm,
turns included. IEC 60092-352 (copyrighted, numbers only): clause 3.19, supports at most 400 mm apart
on horizontal tray runs, fixing points up to 900 mm.

**15. Open wiring and cords at work (OSHA, 29 CFR 1910.305).** Source: the eCFR API, Title 29,
section 1910.305, current as of 2026-10-29,
https://www.ecfr.gov/api/versioner/v1/full/2026-10-29/title-29.xml?part=1910&section=1910.305.
Quotation, (a)(4)(ii), supports for open wiring on insulators: "At intervals not exceeding 1.37 m
(4.5 ft)", within 152 mm (6 in) of a tap and 305 mm (12 in) of a dead end. (a)(2)(xi) asks cords
and cables be supported so they are protected, with no number.

**16. Wiring in aircraft (FAA Advisory Circular 43.13-1B, chapter 11).** Source:
https://www.faasafety.gov/files/events/SO/SO35/2020/SO35100907/AC_43.13_Chapter_11_Electrical.pdf
(an FAA-hosted copy of the chapter). Date: 9/8/98, with change pages dated 9/27/01. Guidance, not a
rule for anything but aircraft repair. Quotation, 11-146: clamps "should be spaced at intervals not
exceeding 24 inches". 11-118: no more than 1/2 in of sag between supports.

**17. Wiring harnesses in spacecraft (NASA-STD-8739.4A with Change 4).** Source:
https://standards.nasa.gov/sites/default/files/standards/NASA/A/4/nasa-std-87394a_w_change_4_0.pdf
(record: https://standards.nasa.gov/node/283). Date: 06/30/2016, change 04/13/2022. Quotation,
7.2.19: "The bend radius data given in Table 7-1 shall apply" to bending in the installed harness or
cable. Table 7-1, optimum then minimum radius as multiples of the outside diameter: individual
coaxial cable 10, 6; polyimide (Kapton) insulated 15, 10; a harness with coax or AWG 8 and larger 10,
6; a harness of AWG 10 and smaller without coax 10, 3; a harness containing polyimide wire 15, 10.
Table 9-1, the farthest apart spot ties may be: harness 6.4 mm or less 19.1 mm; 12.7 mm 38.1 mm;
25.4 mm 50.8 mm; over 25.4 mm 76.2 mm. 7.2.7: redundant systems are not routed in the same bundle.
No number for clamping a harness to structure and none for power-to-signal separation.

**18. Data cable supports.** UFGS 27 10 00 (finding 20): J-hooks on 1.2 to 1.5 m (4 to 5 ft)
centres, at most 20 cables of 6.4 mm per support, cable not touching pipes or ducts. Panduit's
installation guide PN390B (finding 20, copyrighted, numbers only): J-hooks at 5 ft at most, at least
3 in above a suspended ceiling, 12 in clear above ceiling trays.

### Cable and conduit bend radius

**19. Power cable makers' radii (Southwire, data sheets SPEC 10028 and SPEC 10000).** Sources:
https://cabletechsupport.southwire.com/tile/30/spec/10028/?country=US (Romex SIMpull NM-B, revision
1.000.052, "Updated On: Oct. 7, 2025") and
https://cabletechsupport.southwire.com/tile/30/spec/10000/?country=US (SIMpull THHN/THWN-2, revision
1.000.107, "Updated On: Oct. 30, 2026"). Copyrighted: numbers only, as the research pass transcribed
them from the sheets' tables ("Min Bending Radius" against "Approx. OD", inches). NM-B: 14/2 0.7
(flat cable 0.372 by 0.176), 12/2 0.8 (0.422 by 0.190), 10/2 0.9 (0.505 by 0.218), 14/4 1.4 (round
0.35), 8/3 2.3 (0.585), 2/3 3.5 (0.887). THHN: 8 AWG 0.9 (0.217), 4 AWG 1.3 (0.324), 4/0 2.5 (0.618),
500 kcmil 3.6 (0.908). Neither sheet states a multiple or cites a code section; our arithmetic puts
every row near 4 times the diameter (the thickness of a flat cable). Whether that equals the NEC's
multiple is not known (finding 42).

**20. Data cable (UFGS 27 10 00, Building Telecommunications Cabling System; and Panduit).**
Source: https://www.wbdg.org/FFC/DOD/UFGS/UFGS%2027%2010%2000.pdf. Date: "UFGS-27 10 00 (May 2025)".
A DoD contract specification, not law, and not the TIA table it parallels. Quotations: for UTP,
"bend radii must not be less than four times the cable diameter"; for fibre, "must not be less than
20 times the diameter during pulling" and 10 times after installation; and cable "parallel to power
conductors must have a minimum separation of 300 mm 12 in". Panduit PN390B, TX6A 10Gig UTP
installation guidelines (copyrighted, numbers only; read from a distributor's copy,
https://res.cloudinary.com/sonepar-fr/raw/upload/s--_XUcoVDH--/v1/documents/156/3006.pdf, marked
"© Panduit Corp. 2010", saying it follows TIA-568-C and 569-C): bend radius 4 times the cable
diameter; conduit bends at least 6 times the conduit's diameter; no more than two 90-degree bends
between pull boxes; pull at most 25 lbf; for separation from power it refers to the NEC and gives no
distance.

**21. Fibre cable (Corning eCatalog).** Sources:
https://ecatalog.corning.com/optical-communications/US/en/Fiber-Optic-Cables/Indoor/Indoor-Riser-Cables/MIC%C2%AE-Tight-Buffered-Cable%2C-Riser/p/008K81-31130-24
and the ClearCurve Compact Drop riser page (part 001U21-35131-ES). Date: none shown. Copyrighted:
numbers only. An 8-fibre tight-buffered riser cable of 5.55 mm: minimum bend radius 83.25 mm while
installing and 55.5 mm in operation (our arithmetic: 15 and 10 times its diameter); a bend-insensitive
2.9 mm cable: 5 mm either way.

**22. Shipboard cable (IEEE 45.8-2016 and IEC 60092-352).** Sources as finding 14. Copyrighted:
numbers only, as multiples of the cable's diameter D. IEEE 45.8, 6.9, Table 12, cable to 2 kV:
unarmoured under 25 mm 4D, over 25 mm 6D, braid armour 6D, other armour 7D; Table 13 (2 to 35 kV):
single-conductor tape shield 12D, multiconductor tape shield 7D, unarmoured braid shield 6D, armoured
7D; fibre 20 times while installing and 10 in operation (6.2.1); conduit fill 0.40 (0.31 for two
cables, 0.53 for one, 6.6); expansion loops at least 12 times the largest cable's diameter. IEC
60092-352 Table 4, to 1.8/3 kV: unarmoured to 25 mm 4D (6D circuit-integrity), over 25 mm 6D,
armoured or sheathed 6D, laminate-tape screened 8D, sector conductors 8D, mineral 6D; Table 4A, 3.6/6
kV and up: single core 12D, three-core 9D; 3.21, a conduit's bend at least the cable's and at least
twice the pipe's diameter over 63 mm; space factor 0.4 at most.

**23. Aircraft wire bundles (FAA AC 43.13-1B, 11-117).** Source as finding 16. Quotation: bundle
bends "must not be less than 10 times the outside diameter of their largest wire"; 6 times at a
breakout.

**24. Hand-bent EMT conduit (Klein Tools, bender specifications).** Sources:
https://www.kleintools.com/catalog/conduit-benders-and-parts/iron-conduit-bender-full-assembly-12-inch-emt-angle-setter
and the same address with "34-inch" and "1-inch" in place of "12-inch". Date: none shown.
Copyrighted: numbers only. "Centerline Bend Radius (Inch)": 1/2 in EMT (51603) 4.625; 3/4 in
(51604) 5.5; 1 in (51605) 7.375. The benders are marked for 10, 22.5, 30, 45 and 60 degrees; the
angle setter stops at 10, 22.5, 30 and 45. (A retailer's figures for another maker were seen and not
used.)

**25. Liquidtight flexible metal conduit (Electri-Flex, Liquatite Type LA).** Source:
https://www.electriflex.com/products/type-la/ (page metadata dateModified 2026-09-09). Copyrighted:
numbers only. Inside bend radius, static then dynamic: 1/2 in 3.0, 5.0 in; 3/4 in 4.2, 6.0 in; 1 in
5.5, 12.0 in. The maker's own metric table disagrees (1 in static 165 mm, 6.5 in), so this table is
internally inconsistent and is not used for data rows until the maker is asked.

**26. How a rigid run changes direction (UFGS 26 20 00, Interior Distribution System).** Source:
https://www.wbdg.org/FFC/DOD/UFGS/UFGS%2026%2020%2000.pdf. Date: "UFGS-26 20 00 (May 2025)", Change
2, 02/26. Quotations, 3.1.6.13: "Make changes in direction of runs with symmetrical bends or
cast-metal fittings", field bends made with a hickey or a conduit-bending machine, no crushed or
deformed conduit. 3.1.6: where conduit will be visible, "Install conduit parallel with or at right
angles to ceilings, walls" and structural members. (The NEC's limit on bends between pull points,
358.26, was not read: finding 42.)

### Separation and clearances

**27. Communications next to power (OSHA, 29 CFR 1910.308(e)).** Source: the eCFR API, Title 29,
section 1910.308, current as of 2026-10-29. Quotation, (e)(2)(iv): "Indoor communications wires and
cables shall be separated at least 50.8 mm (2 in.)" from light, power, Class 1 and non-power-limited
fire alarm conductors, unless "a special and equally protective method" is used. The same 2 in for
power-limited fire alarm circuits in (d)(3)(iii); 6 ft (1.83 m) from lightning conductors in
(e)(2)(ii). This mirrors the NEC's communications rule, which was not read.

**28. The space over a panel (OSHA, 29 CFR 1910.303(g)).** Source: the eCFR API, Title 29, section
1910.303 ([46 FR 4056, Jan. 16, 1981, as amended at 73 FR 64205, Oct. 29, 2008]). Quotation,
(g)(1)(vii)(A)(1): the space the width and depth of switchboards and panelboards, from the floor to
6.0 ft (1.83 m) above them or to the structural ceiling, is dedicated, and "equipment foreign to the
electrical installation may not be located in this area"; above it, foreign systems are kept out
unless protected against condensation, leaks or breaks. Working space: the equipment's width or 30 in
(762 mm), whichever is greater; 6.5 ft (1.98 m) of headroom for installations from 13 August 2007;
depth 3 ft (0.9 m) at 0 to 150 V, 3, 3.5 or 4 ft at 151 to 600 V by condition (Table S-1).

**29. Piping over switchboards aboard ships (46 CFR 56.50-1, 111.30-5, 111.01-3).** Source: the eCFR
API, Title 46, Parts 56 and 111, current as of 2026-10-28; 56.50-1 as amended at 89 FR 50131, June
12, 2024. Quotation, 56.50-1(d): "Piping must generally not be run over switchboards", and is kept as
far from other electrical equipment as practicable, guarded against leaks where it cannot be.
111.30-5(b): each switchboard is fitted with a dripshield (88 FR 16361, Mar. 16, 2023); 111.01-3(a):
electrical equipment is arranged to avoid damage from dripping liquids.

**30. Pipe and wire through the same hole (24 CFR 3280.603(b)(7)).** Source as finding 8; 3280.603
last amended at 89 FR 75751, Sept. 16, 2024. Quotation: "Piping and electrical wiring shall not pass
through the same holes". 3280.603(b)(1): structural members are not weakened by careless cutting or
notching. 3280.806(e) (Subpart I): no receptacle within reach (30 in) of a shower or bathtub space.

**31. Conduit near hot pipes, and shared supports (UFGS 26 20 00).** Source as finding 26.
Quotations, 3.1.6: "Keep conduit minimum 150 mm 6 inches away from parallel runs of flues" and steam
or hot water pipes; and, of supports, "Do not share supporting means between electrical raceways and
mechanical piping or ducts". Exposed risers are clamped at each floor and every 3050 mm (10 ft) at
most; plastic cable ties are not acceptable supports.

**32. Wiring and flammable fluid lines in aircraft (FAA AC 43.13-1B, 11-126).** Source as finding
16. Quotations: wiring is routed above lines carrying flammable fluids or oxygen "with a minimum
separation of 6 inches or more whenever possible", and otherwise "A minimum of 2 inches must be
maintained", or 1/2 in where the wire is positively clamped; never run parallel to such a line.
11-125: 1/2 in from movable controls.

**33. Separation aboard ships and spacecraft (IEEE 45.8; SSP 30242F).** IEEE 45.8 (copyrighted,
numbers only): cables carrying high-amplitude pulses 610 mm (24 in) from other services (6.4.1 k);
propulsion power cables 610 mm from ship-service, control and signal cables (6.4.3); keep cables away
from drip, water, oil, steam and hot pipes, with no distance given. NASA SSP 30242 Revision F (31 July
2002; read from https://everyspec.com/NASA/NASA-JSC/NASA-SSP-PUBS/download.php?spec=SSP_30242F.028771.pdf,
a third-party host) gives no fixed distance for the International Space Station: the minimum
edge-to-edge separation between bundles, quotation, "for parallel runs of length L (in feet) shall
be calculated" to reach 20 dB of isolation by a method in SSP 30243 (not read), between circuit
classes kept in separate bundles.

**34. PEX near heat (the PPI guide, finding 3).** Numbers only: at or below 180 F; IC-rated recessed
lights reach up to 194 F at their surface and non-IC up to 300 F; 6 in from a gas or oil water
heater's flue, with 18 in metal connectors at the heater.

**35. Cable next to a water pipe, generally: no distance found (negative).** No source read gives a
general distance between an electrical cable and a water pipe. 24 CFR 3280 Subpart I has none
(searched); England's Approved Document P (2013,
https://assets.publishing.service.gov.uk/government/uploads/system/uploads/attachment_data/file/441872/BR_PDF_AD_P_2013.pdf)
gives none, only the "special location" round a bath or shower and a note keeping socket-outlets 3 m
from a bath or shower tray; IEEE 45.8 says keep away from drip and water with no distance; the NEC was
not read. What the sources do give is narrower: no shared holes (finding 30), no shared supports
(31), nothing above panels (28, 29), wiring above fluid lines (32).

### Slope

**36. Drain slope in a manufactured home (24 CFR 3280.610(h), 3280.606).** Source as finding 8;
last amended at 86 FR 2521, Jan. 12, 2021. Quotation, 3280.610(h): horizontal drainage piping has "a
uniform grade of not less than 1/4 inch per foot" toward the drain outlet; where that is
impractical, 1/8 in per foot with a full-size cleanout at the upper end. 3280.606: a trap arm falls
1/4 in per foot toward the vent, by no more than its own diameter.

**37. Drain slope in the model codes (IPC 2009 Table 704.1; UPC 2009 708.0).** Sources as findings
11 and 12; copyrighted, numbers only. IPC 2009: 2 1/2 in pipe or smaller 1/4 in per foot; 3 to 6 in
1/8 in per foot; 8 in or larger 1/16 in per foot. UPC 2009: at least 1/4 in per foot (2%); 4 in and
larger may go to 1/8 in per foot (1%) where 1/4 is impractical, with the authority's approval.

**38. Which drain fitting turns which way (IPC 2009 Table 706.3).** Source as finding 11; numbers
and rules only. A sanitary tee turns only from horizontal to vertical; a quarter bend turns vertical
to horizontal and horizontal to horizontal only on a fixture drain of 2 in or smaller; a short
sweep turns vertical to horizontal at 3 in or larger or on a 2 in or smaller fixture drain;
sixteenth, eighth and sixth bends, long sweeps, wyes and combination wye and eighth bends may turn in
all three directions.

### Through walls, decks and studs

**39. Penetrations of bulkheads and decks aboard ships (46 CFR 56.50-1(a), (b)).** Source as finding
29. Quotation, 56.50-1(a): where piping passes through watertight, oiltight or fire bulkheads and
decks, "the integrity of the structure must be maintained"; no lead or other metal melting at 1,700 F
or less where fire would impair the penetration; openings reinforced. (b): a pipe through the
collision bulkhead is welded on both sides, with a valve worked from above the bulkhead deck.

**40. Cable through bulkheads and fire stops aboard ships (46 CFR 111.60-2; IEEE 45.8; IEC
60092-352).** Source: the eCFR API, Title 46, section 111.60 (88 FR 16363-4, Mar. 16, 2023).
Quotation, 111.60-2, for specialty cable that fails the flame test: fire stops "At least every 7
meters (21.5 feet) vertically", at most two deck heights, every 15 m (46 ft) horizontally and at each
A or B class boundary. IEEE 45.8 6.5.1 (numbers only): a bulkhead 6 mm or thicker may omit the
bushing if the hole's edges are rounded. IEC 60092-352 3.20: watertight glands or transits.

**41. Through studs and framing.** 24 CFR 3280.808(e) (source as finding 13): cable may pass
through the centre of the wide side of 2 by 4 studs, with steel plates or a tube of at least No. 16
MSG where it would be less than 1 1/2 in from a stud's face. IPC 2009 305.8 (finding 11): shield
plates within 1 1/2 in for pipe. UPC 2024 312.9, as the PPI guide reprints it (finding 3): plates
within 1 in. The PPI guide: holes 1/4 in larger than the tube.

### Access to the codes themselves

**42. The National Electrical Code (NFPA 70): not read (negative).** Source:
https://www.nfpa.org/for-professionals/codes-and-standards/list-of-codes-and-standards/free-access
(page metadata updatedDate 2025-09-16). Free reading needs an NFPA account, terms accepted in a
read-only viewer, no download or print. The research did not create an account, so 334.24 (NM bend
radius), 334.30 (NM supports), Chapter 9 Table 2 (conduit bends), 800.133 (communications
separation, or its 2023 number), 110.26(E) (dedicated space) and 358.26 (bends between pull points)
were not read. Public.Resource.Org's incorporated-by-reference folder
(https://law.resource.org/pub/us/cfr/ibr/inc/nfpa/) holds other NFPA documents but no NFPA 70. Federal
law incorporates particular editions: 46 CFR 110.10-1 the 2017 edition, 24 CFR 3280.4 the 2014
edition. No state or city publication of the NEC's text was found.

**43. The current plumbing codes: not read (negative).** The ICC's reader
(https://codes.iccsafe.org/content/IPC2024P1/chapter-7-sanitary-drainage) answered scripted requests
with a 403 and, with a browser's user agent, a page of scripts and a captcha, as it did for the
2026-10-20 stairs finding. IAPMO's reader (https://epubs.iapmo.org/2024/UPC/) is a script
application that returned only its title. Ohio's administrative code
(https://codes.ohio.gov/assets/laws/administrative-code/rules/4101/3/4101$3-7-01_eff_10_15_25.pdf)
lists only its amendments to the 2021 IPC. So the 2024 slope and hanger tables were read only where
the PPI guide reprints them (PEX).

**44. Standards that are sold, not published: not read (negative).** TIA-568 and TIA-569 (TIA-569-E
"Posted November 19, 2020", sold through Accuris, https://tiaonline.org/standard/tia-569/); SAE J517
(hose, https://www.sae.org/standards/content/j517_202408/); MSS SP-58 (pipe hangers, incorporated by
reference in 46 CFR 56.01-2; preview at https://webstore.ansi.org/preview-pages/MSS/preview_MSS+SP-58-2009.pdf
refused with a 403); ASME B31.1 and B31.9; BS 7671 and IEC 60364.

## What is still unknown, and what would settle it

- **The NEC's numbers** (finding 42): a person with an NFPA account reads 334.24, 334.30, Chapter 9
  Table 2, 358.26, 110.26(E) and the communications separation in the 2023 edition's viewer and
  records them in a new dated finding. Free, about an hour. Until then the game's NM cable radius
  comes from the maker's sheet (finding 19) and is labelled as the maker's figure, not the code's.
- **The 2024 IPC and UPC tables** (finding 43): read in a browser by a person, or from a state that
  republishes the text as law. Free, an hour or two.
- **MSS SP-58 hanger spacing**: buy the standard (sold by ANSI; price not recorded) or rely on UFGS
  23 05 15, which this finding does.
- **Garden and potable-water hose bend radius**: a hose maker's data sheet for the hose the game
  draws. Free.
- **Cable protector ramp load ratings** (the design's ramps): a maker's published rating per tyre
  or axle. Free. Not researched today.
- **Friction loss in fittings** (the design's water network): the CDA handbook's Table 14.7
  (https://copper.org/resource-library/copper-tube-handbook-table-14-7/) gives equivalent lengths of
  standard elbows; its cells ran together in extraction and were not recorded. Read it in a browser.
- **Whether Public.Resource.Org's copies of IEEE 45.8 and IEC 60092-352 may be quoted** (they carry
  the publishers' copyright notices; Public.Resource.Org stamps them as incorporated law): the
  project only cites their numbers here, which needs no permission; reproducing their text would be
  a question for a lawyer.

## Sources that could not be reached

- copper.org handbook PDFs, https://www.copper.org/publications/pub_list/pdf/copper_tube_handbook.pdf
  and https://copper.org/wp-content/uploads/2026/04/copper_tube_handbook_print_press.pdf: 403 to
  scripted requests (the HTML pages loaded with a browser's user agent). Hoped for the edition line
  and Table 4.1's footnote.
- https://archive.copper.org/applications/plumbing/techref/guidespecs/gde_3execution.php: 401.
- https://www.huduser.gov/Publications/PDF/pex_design_guide.pdf (the 2006 PEX guide): empty response.
- The whole PPI third edition: over the 10 MB fetch limit; read by chapter instead.
- Uponor's and Viega's PEX manuals: not found on the makers' sites; the PPI guide names both as
  contributors.
- https://webstore.ansi.org/preview-pages/MSS/preview_MSS+SP-58-2009.pdf: 403.
- https://codes.iccsafe.org/content/IPC2024P1/chapter-7-sanitary-drainage: 403, then a captcha.
- https://epubs.iapmo.org/2024/UPC/: script application, title only.
- archive.org's UPC 2012 item (gov.law.iapmo.upc.2012): "Couldn't locate item".
- eCFR web pages: redirect scripted requests to a bot check (unblock.federalregister.gov). The eCFR's
  public API served the same text, and every CFR quotation here comes from it.
- gates.com hose pages: 403. Parker's TC3 sheet (promo.parker.com): redirects to the home page.
  connectorspecialists.com's copy of the Parker hose section: 404.
- https://www.sae.org/standards/content/j517_202408/: the header only.
- https://www.nfpa.org/codes-and-standards/nfpa-70-standard-development/70: the title only.
- TracPipe FGP-001US 04/22 (https://res.cloudinary.com/dzhpkjoqj/TracPipe/TP-IO_Manual.pdf): over the
  fetch limit. Gastite's Design and Installation Guide: not located (its 2014 specification only
  defers to it). WARDFLEX: not tried.
- Corning's application notes AEN001 and LAN-1232
  (https://www.corning.com/catalog/coc/documents/application-engineering-notes/AEN001.pdf): 403.
- Cerrowire's NM-B data sheet: none found; https://www.cerrowire.com/products/nm-b/ is a 404. Encore
  Wire: only a distributor's copy found, not opened. Tricab's marine cable data: behind a login.
- Greenlee's bender pages (greenlee.com): script-driven or 404.
- SSP 30243 (the isolation method SSP 30242 points to): not searched.
- BS 7671 and IEC 60364: sold; not tried.

## What it means for this project

### Certain

- **Rigid and flexible are different behaviours, and the sources say so in so many words.** Rigid
  PVC and CPVC change direction only with fittings (finding 4); conduit changes direction with
  "symmetrical bends or cast-metal fittings" (26); copper bends only with a bender of a stated radius
  and, hard, only up to 3/4 in in the table read (1); PEX bends freely at 6 times its diameter or
  more (3); hose and cable have a maker's minimum radius (6, 19 to 23). A run's material therefore
  decides its shape, which is the design's three behaviour classes.
- **Today's 90-degree corners are not real for any material read.** Even a fitting's elbow has a
  centreline radius, and every bent tube and cable a minimum radius.
- **Exposed rigid runs really do follow the structure** (26: parallel with or at right angles to
  walls and ceilings), so the v0.536 rule that a run never cuts diagonally through a room stays true
  for rigid pipe and conduit; curves belong to flexible runs and to the bends of rigid ones.
- **The numbers depend on where you are.** A cable is strapped every 4 1/2 ft in a manufactured home
  (13) and every 24 in aboard a small passenger vessel (14); a harness is tied every 19 to 76 mm in a
  spacecraft (17). The rules therefore belong to a rule profile chosen by the place (a home, a ship,
  a spacecraft), not to the cable alone.
- **Several rules are about water landing on electricity**: nothing foreign over a panel (28), no
  piping over switchboards and drip shields on them (29), wiring above fluid lines in aircraft (32).
  That gives the routing's vertical order (power above water) a source, which the old
  `data/routing_rules.ron` header attributed to "NEC 300.4" without a reading; the NEC was not read
  here either (42), so that citation stays unverified and should be replaced by findings 28, 29 and 32.
- **No general cable-to-water distance exists in what was read** (35). The game should not invent
  one and present it as a code rule.

### Judgement calls

- **Which numbers become data rows** (the design's `data/piping/run_rules.ron`, each row citing its
  finding here):
  - bend radius: copper by size and temper (1); PEX 6 times OD and 18 times against the coil where
    a PEX row says so (3); PVC and CPVC "fittings only" (4); hose per hose row (6, and ours for a
    garden hose until a maker's sheet is read); CSST (7); NM-B and THHN about 4 times the diameter,
    labelled as the maker's (19); UTP 4 times, fibre 20 and 10 times (20); ship cable 4D to 12D by
    construction (22); aircraft bundles 10 times (23); EMT by bender (24);
  - supports: per profile, from findings 8, 9, 10, 13, 14, 15, 16, 17 and 18; the residential
    profile uses 24 CFR 3280 where it has a number and UFGS 23 05 15 for metal pipe; copper's
    disagreement (finding 9's table) is shown in the row's note, the profile choosing UFGS for
    metal pipe because it is current and public;
  - support positions: within 0.3 m of each bend, 1.5 m apart at valves, within 0.9 m of a
    runout's end (9, 10); within 0.3 m of a box for cable (13);
  - separation: communications 50.8 mm (27) and 300 mm when parallel (20) as two rows, the profile
    choosing; the 1.83 m dedicated space over panels (28); no piping over switchboards (29); no
    shared holes (30); no shared supports (31); 150 mm from hot pipes and flues (31); wiring above
    fluid lines 152 mm whenever possible, 51 mm minimum, 13 mm clamped (32);
  - slope: 1/4 in per foot, 1/8 in for larger pipe or with a cleanout (36, 37); drain fittings by
    direction (38);
  - penetrations: sleeves or plates near stud faces (41), bulkhead integrity and heat-resistant
    materials (39), glands or transits for watertight bulkheads and fire stops for some cable (40);
  - fill and pull limits for conduit: 0.40 fill (22), two 90-degree bends between pull points for
    data conduit (20).
- **Which profile the ship uses.** Aboard a ship the US vessel rules (46 CFR and IEEE 45.8, which 46
  CFR incorporates) are the closest real body of rules; the home inside the ship is a dwelling, for
  which 24 CFR 3280 is public federal law. The design proposes a ship profile for the ship's
  services and a dwelling profile for a home's own wiring, each stated in the editor.
- **What counts as a "fault" against a "code" finding** in the editor is the design's call; the
  sources only say what a rule is.

This reading is not legal or engineering advice. Nobody who wrote it is a lawyer, an electrician, a
plumber or a building official, and anyone installing a real pipe or cable must follow the edition
of the code their own authority has adopted, read in full.
