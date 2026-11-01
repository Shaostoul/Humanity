# How must a powered door, exit or hatch open by hand when its power fails?

**Research date: 1 November 2026.** Everything below was read on that day. If you are reading this
later, check each source's own date: the US regulations are amended from time to time (the bus
standard read here was amended in June and July 2026), and the elevator codes publish new editions.

This is a reading of public sources by an AI working on the project, not legal or engineering
advice. Nobody who wrote it is a lawyer, a rail, aviation or marine engineer, or an elevator
inspector. The people who decide how these rules apply to a real vehicle or building are its
regulator and inspector, working from the edition in force.

Companion finding, same day: [2026-11-01-pipe-and-cable-routing-rules.md](2026-11-01-pipe-and-cable-routing-rules.md).
The design that uses this: [docs/design/spline-routing.md](../../design/spline-routing.md), section 13
(tapping into infrastructure: a powered door's emergency release panel).

## The question

When a powered door, emergency exit or hatch (on a train, an aircraft, a bus, a lift, a ship and a
spacecraft) loses its power, what do primary sources require of the way a person opens it by hand:
where the release is, whether it needs a tool, whether it sits behind a removable cover, which
sides it works from, how it is marked, and who may use it?

## How quotations are handled here

The US regulations, the US state rules and the NASA documents are government text and are quoted
exactly, under 15 words. The UK regulators' pages are Crown copyright under the Open Government
Licence and are quoted briefly. The Dutch government's checklist is quoted not at all (it
paraphrases SOLAS, which was not read). Paywalled standards are listed as negative findings.

## Short answer

Across rail, aircraft, buses, ships and spacecraft the rule is the same in substance: **a powered
door or exit has a manual way to open it that works without power, sits at the door, needs no tool,
works from both sides (or from inside and outside), and is marked with instructions.** Rail goes
furthest toward the game's idea: the release may sit behind a cover "capable of removal without
requiring the use of a tool", and interior doors carry a removable panel a person can push out.
Ships give sliding watertight doors hand gear (a crank) beside their power. Pressure hatches add two
separate actions to unlatch and a way to equalize pressure by hand from each side. **Lifts are the
exception:** a landing door is unlocked from outside only with a special key, by trained and
authorised people, after the lift's power is switched off and locked; California bans the unlocking
device outright.

| Where | Release without power | Tool | Sides | Cover or panel | Marking | Who |
|---|---|---|---|---|---|---|
| Passenger rail (49 CFR 238.112) | yes, a manual override adjacent to the door | none | inside; both for newer cars | a cover or screen removable without a tool; interior doors also a removable panel | luminescent, with instructions | anyone |
| Aircraft (14 CFR 25.809, 25.811) | manual operation acceptable after a power system fails | none ("simple and obvious") | inside and outside | (not addressed) | handle and instructions readable at 30 in | anyone |
| Buses (49 CFR 571.217) | school buses: notwithstanding a power failure | none | inside and outside (school buses) | (not addressed) | instructions within 15 or 16 cm of the release | anyone |
| Lifts (state rules, UK HSE) | an unlocking key, power off and locked first | the key | outside the landing | the key kept beside the disconnect, or in a locked box | a label on the key | trained, authorised people only |
| Ships (46 CFR 170.250, 174.100, 116.500) | power sliding doors also have hand gear | (hand gear) | both sides | (not addressed) | (not addressed) | one person |
| Spacecraft (NASA-STD-3001 Vol. 2 Rev F) | hatches operable by hand | none | either side | (not addressed) | latch state and pressure difference shown on both sides | a single crewmember |

## Findings

### Passenger rail

**1. Door emergency release (49 CFR 238.112).** Source: the eCFR API, Title 49, section 238.112,
current as of 2026-10-29,
https://www.ecfr.gov/api/versioner/v1/full/2026-10-29/title-49.xml?part=238&section=238.112.
Date: [78 FR 71812, Nov. 29, 2013], no later change shown. Quotations: (b)(1), the override lets the
door be "opened without power from both inside and outside the car" (Tier I cars ordered under the
newer rules, and all Tier II cars); (c), the override may be protected by a "cover or a screen
capable of removal without requiring the use of a tool". In summary: (a) a powered exterior side
door has a manual override adjacent to the door that releases it to open without power from inside,
with no tool or other implement; (d) and (e) doors are marked and carry instructions (since 28
January 2015, under 238.125); (f) for cars ordered from 28 January 2014, each vestibule and interior
door has a removable panel or window for when it will not open, removable rapidly and easily from
each side without tools, at least 21 in wide by 28 in high with its lowest point no more than 18 in
above the floor; a powered interior door has a manual override from each side without a tool; a
bi-parting door has a hold-open device as well; panels and overrides are marked with luminescent
material and instructions on each side; a sample is tested at least every 184 days.

**2. Emergency windows and rescue access (49 CFR 238.113, 238.114).** Source: the same API,
sections 238.113 and 238.114. Date: [73 FR 6401, Feb. 1, 2008, as amended at 78 FR 71813, Nov. 29,
2013]. Quotation, 238.114: responders remove a rescue access window "without unreasonable delay",
with a provided external mechanism or common responder tools. Summary: 238.113 emergency windows come
out rapidly and easily from inside without tools, at least 26 by 24 in, marked luminescent with
instructions; 238.114's outside rescuers may use tools, the one case where a tool is expected.

**3. Marking (49 CFR 238.125) and the APTA standard it names.** Source: the same API, section
238.125 ([78 FR 71814, Nov. 29, 2013]). It follows APTA PR-PS-S-002-98 Rev. 3, incorporated by
reference; that standard returned a 403 and was not read (see below).

**4. Emergency preparedness plans (49 CFR Part 239): negative.** The whole part, as current in the
eCFR (latest citation 90 FR 28189, July 1, 2025), has no occurrence of "door"; it covers exits only
through drills and procedures.

### Aircraft

**5. Emergency exit arrangement (14 CFR 25.809).** Source: the eCFR API, Title 14, section 25.809.
Date: last amended by Amendment 25-116, 69 FR 62788, Oct. 27, 2004. Quotations: (b) "Each emergency
exit must be openable from the inside and the outside"; (d) where one power system opens several
exits, "Manual operation of the exit (after failure of the primary system) is acceptable." Summary:
(b) an exit opens within 10 seconds, in any attitude a landing-gear collapse leaves, with people
crowded against it; (c) the means of opening is simple and obvious, needs no exceptional effort, and
can be found and worked in darkness; (i) an open exit holds itself open without a separate action
and closes only by positive action.

**6. Emergency exit marking (14 CFR 25.811).** Source: the same API, section 25.811; last amended by
Amendment 25-88, 61 FR 57958, Nov. 8, 1996. Summary: the handle's location and the opening
instructions are readable from 30 in; the handles of larger exits are self-illuminated (at least 160
microlamberts) or well lit; a rotary handle carries a red arc arrow and the word "open" in red
letters 1 in high; outside, a 2 in contrasting band outlines the exit and its means of opening.

**7. Emergency exit access (14 CFR 25.813).** Source: the same API, section 25.813; last amended by
Amendment 25-128, 74 FR 25645, May 29, 2009. Summary: a removable hatch's placard states its weight
and where to put it once removed.

### Buses

**8. Bus emergency exits (FMVSS 217, 49 CFR 571.217).** Source: the eCFR API, Title 49, section
571.217 (human page:
https://www.ecfr.gov/current/title-49/subtitle-B/chapter-V/part-571/subpart-B/section-571.217).
Date: source [37 FR 9395, May 10, 1972]; the eCFR's history shows substantive amendments dated
2024-10-30, 2024-12-30, 2026-06-03 and 2026-07-06, not checked for what they changed. Quotation,
S5.3.3.1 (school bus emergency doors): the door opens from inside and outside "notwithstanding any
failure of the vehicle's power system". Summary: other buses (S5.3.2) are released by one person by
hand from inside with at most two release mechanisms, within force limits (90 N in the low-force
region, 270 N in the high-force region); a single mechanism needs two motions whose directions
differ by 90 to 180 degrees; school bus doors need no remote control or tool, open with no more than
178 N, and sound a warning when unlatched with the ignition on; labels say "Emergency Door" or
"Emergency Exit" with motion-by-motion instructions within 16 cm (15 cm on school buses) of the
release, legible to 20/40 vision under night lighting.

### Lifts

**9. Hoistway door unlocking keys (Wisconsin Administrative Code SPS 318.1702).** Source:
https://docs.legis.wisconsin.gov/document/administrativecode/SPS%20318.1703(1)(b) (the page shows
this stretch of chapter SPS 318). Date: CR 14-020, eff. 9-1-14; CR 19-118, eff. 6-1-20. Quotation,
the label the key must carry: "Place the disconnecting means in the off position and lock it"
before using it. Summary: the hoistway door unlocking key is kept beside the elevator's disconnect;
firefighters' key boxes open only for the fire department, police, licensed elevator inspectors and
other authorised people (318.1702(10)(b)3.a). The state rule sits on top of ASME A17.1 section
2.12.6.2, which was not read.

**10. Emergency release keys under strict control (UK Health and Safety Executive, Safety Alert
OPSTD 2-2012).** Source: https://www.hse.gov.uk/safetybulletins/liftingplatforms.htm. Date: issued
18 June 2012. Quotation: emergency landing-door keys are to be used "only in exceptional
circumstances and by suitably trained and authorised people". Summary: two incidents, one fatal,
followed care staff using the keys in daily use: the doors opened onto the well while the platform
was at another level. Keys under strict control, and procedures for releasing trapped people.

**11. California bans the unlocking device (Title 8, sections 3141.7 and 3021).** Sources:
https://www.dir.ca.gov/title8/3141_7.html (filed 4-1-2008, operative 5-1-2008) and
https://www.dir.ca.gov/title8/3021.html (last history entry filed 7-1-91). Quotation, 3141.7(a)(1):
the hoistway door unlocking devices described in ASME A17.1-2004 section 2.12.6 "are prohibited on
all conveyances". Summary of 3021: a hoistway door may be unlockable from outside only when the car
is within 30 in of the landing; the access key goes only to inspectors, maintenance staff and
repairers; in passenger lifts installed after 1 January 1989, doors outside the unlocking zone open
no more than 4 in from inside the car.

### Ships

**12. Watertight door classes (46 CFR 170.250, 170.270).** Source: the eCFR API, Title 46, Part 170,
Subpart H (CGD 79-023, 48 FR 51010, Nov. 4, 1983; 170.270 last amended 65 FR 58464, Sept. 29, 2000).
Quotation, 170.250: "Class 3—Sliding door, operated by power and by hand gear." (the dash is the
source's). Summary: Class 2 doors are hand gear only; 170.265 says where Class 3 doors are
required; 170.270: Class 1 hinged doors have a quick-acting closer worked from both sides; Class 2 and
3 doors meet ASTM F1196 and F1197 (not read, below); the pilothouse shows each door open or shut.

**13. Watertight doors on offshore drilling units (46 CFR 174.100).** Source: the same API, section
174.100 ([CGD 79-023, 48 FR 51048, Nov. 4, 1983, as amended ... 65 FR 58464, Sept. 29, 2000]).
Quotation, (c)(1)(i): watertight doors used at sea are remotely controlled and also "operable
locally from both sides of the bulkhead". Without remote control, an open or shut alarm.

**14. Fire doors and escape doors on small passenger vessels (46 CFR 116.435, 116.500).** Source:
the same API, Part 116; 116.500 last amended USCG-2021-0306, 86 FR 73171, Dec. 27, 2021.
Quotations: 116.500(i), an escape door, hatch or scuttle is "capable of being opened by one person,
from either side" in light and dark; 116.435(c)(6)(ii), a self-closing door's hold-back in a
stairtower or main vertical zone bulkhead releases locally, on a signal from a control space, and
"Upon disruption of the power system." Summary: so a held-open fire door shuts when the power fails;
116.435(b)(2), a door in a fire control boundary is worked from either side by one person; 116.500(i), the method is
obvious, rapid and of adequate strength, with handles permanently fitted; 116.435(c)(7), horizontal
doors (doors set in decks) open with at most 17.8 N (5 lb) to release the latch, 44.5 N (10 lb) to
start moving and 17.8 N to open fully.

**15. Doors on passenger vessels (46 CFR 72.05-25).** Source: the same API, Part 72 (CGFR 65-50,
30 FR 16903, Dec. 30, 1965; the section's own amendment bracket not confirmed). Quotation, (a)(1):
doors are "capable of operation from either side by 1 person".

**16. UK passenger ship instructions (Maritime and Coastguard Agency, MSIS003 Part 3, Rev 1.01,
section 3.1.2.1).** Source:
https://assets.publishing.service.gov.uk/media/5a7c8af740f0b626628acccb/con1_2a_pt3.pdf. Date: not
shown beyond "REV 1.01". Quotation: power sliding watertight doors are "released locally, from both
sides of the bulkhead". Summary: their hand gear is permanently engaged, normally a crank handle or
wheel, and also works from above the bulkhead deck.

**17. SOLAS, as a Dutch government checklist paraphrases it (secondary).** Source:
https://puc.overheid.nl/doc/PUC_2185_14?exp=2 (valid from 1 January 2011). Not quoted, because it
paraphrases SOLAS regulation II-1/13 as yes/no questions: a hand mechanism at the door on either
side and above the bulkhead deck (within 90 seconds from above it); no single failure of the
electric or hydraulic system may prevent hand operation; control handles on each side at least
1.6 m above the floor. The IMO's consolidated SOLAS text was not reached, so these stay secondary.

### Spacecraft

**18. Hatches (NASA-STD-3001 Volume 2, Revision F, section 8.4).** Source: the record
https://standards.nasa.gov/node/237 (the PDF sits under a temporary path; fetch it through the
record). Date: "Approved: 2026-07-14", superseding Revision E. A NASA programme standard, not law.
Quotation, [V2 8022]: hatches and doors are "operable on either side by a single crewmember without
the use of tools". Summary: [V2 8023] two distinct, sequential operations to unlatch; [V2 8024]
operable within 60 s from both sides in normal use; [V2 8025] operating force within crew strength
at the worst-case pressure difference; [V2 8028] manual pressure equalization from each side; [V2
8029] a window to see across; [V2 8031] and [V2 8032] latch state and pressure difference shown on
both sides. The rationale cites Mir in 1997, when the crew could not close a hatch against the
rushing air.

**19. How the station's hatches open (NASA OCHMO Technical Brief TB-028, "Vehicle Hatches").**
Source: https://www.nasa.gov/wp-content/uploads/2023/12/ochmo-tb-028-vehicle-hatches.pdf. Date:
11/29/2023 (keyed to Revision D). Summary only: the crew opens the manual pressure equalization valve
first, works the latch drive with a crank (counterclockwise inside, clockwise outside), selects
unlatch, stows the crank and swings the hatch open by hand.

## What is still unknown, and what would settle it

- **The rail marking standard** (APTA PR-PS-S-002-98 Rev. 3): it appears to be a free PDF at
  apta.com that refused scripted requests; a person downloading it in a browser settles what the
  markings look like. Free.
- **ASME A17.1 and EN 81-20** (lifts): sold (search results showed ASME's PDF at $335 and a CSA
  listing at $460; EN 81-20 from about 210 to 290 euros). The state rules above cover the point the
  game needs (keys, trained people, power isolated first).
- **ASTM F1196 and F1197** (sliding watertight doors and their controls, incorporated by reference in
  46 CFR 170.270): sold (F1197-19 listed at $86); its free abstract says each sliding watertight door
  has local and remote manual operation.
- **SOLAS II-1/13 itself**: the IMO's consolidated text is sold; a flag state's republication would
  settle finding 17.
- **What the 2026 amendments to FMVSS 217 changed**: read the Federal Register notices of 2026-06-03
  and 2026-07-06. Free.

## Sources that could not be reached

- https://www.apta.com/wp-content/uploads/Standards_Documents/APTA-PR-PS-S-002-98.pdf and the
  buyersguide.apta.com copy: 403. Hoped for the rail marking's look.
- ASME A17.1-2025/CSA B44 and EN 81-20:2020: sold; an asme.org product page returned 404.
- ASTM F1196 and F1197: sold; https://www.astm.org/f1197-23 returned 403.
- The IMO's SOLAS text and resolution MSC.216(82): not found free.
- 524 CMR 2.12.6 (Massachusetts) on regulations.justia.com: 403.
- BS 7255 and the lift industry's guidance sheets: not read.
- No OSHA source on elevator emergency unlocking was found.

## What it means for this project

### Certain

- **A powered door with a manual release is how the real world builds them.** Every class of
  vehicle read, apart from lifts, requires a release that works without power, at the door, with no
  tool, from both sides, marked with instructions. The game's emergency release panel on a powered
  door copies a real rule, not a game convenience.
- **Rail allows exactly the panel the operator described:** a release behind a cover removable
  without a tool (finding 1), and on interior doors a removable panel to get through when the door
  will not open.
- **Ships pair power with a crank.** The existing `Crank` lock interaction ("hold E to wind it open",
  `data/blueprints/lock_types.ron`) is the real hand gear of a Class 3 sliding watertight door
  (finding 12).
- **A held-open fire door shuts when power fails** (finding 14), and an exit held open stays open
  until a positive action shuts it (finding 5): two real fail directions, by door type.
- **Lift landing doors are the deliberate opposite:** a key, trained and authorised people, and the
  lift's power switched off and locked first (findings 9 to 11).

### Judgement calls

- **The game's release panel should follow rail and ships:** a marked cover that comes off without a
  tool on each side of every powered door, behind it a release that declutches the drive, then the
  door slides by hand or by its crank (the existing `Crank` turns), with the force and time as data.
- **Pressure doors and airlocks should follow NASA:** two separate actions to unlatch, and a manual
  equalization valve that must be opened first; a hatch will not open against a pressure difference.
- **Lift landing doors should stay locked to everyone without the key and the training**, with the
  car's power isolated first, which keeps the existing rule that a landing door never opens onto an
  empty shaft (`home_meshes::door_target`) true by a real mechanism.
- **Which way a door fails on power loss** (shut, open, or where it stands) is a per-door-type data
  row; `docs/design/locks.md` records that choice as the operator's (its first open fork). These
  findings show that real codes choose by door type, and that a manual release always exists.

This reading is not legal or engineering advice. Nobody who wrote it is a lawyer, an engineer or an
inspector.
