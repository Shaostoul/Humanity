# Where should a wood stove in a tight home get the air its fire burns?

**Research date: 20 October 2026.** Everything below was read on that day. If
you are reading this later, check the date of each source against anything that
has changed since: the eCFR text is updated whenever HUD amends the rule, and
Drolet revises its manuals.

This is a reading of public sources by an AI working on the project, not legal
advice. Nobody who wrote it is a lawyer, a building official or a certified
installer. The people who decide how the rules apply to a particular home are
the local building official and fire marshal, and the stove's own installation
manual and certification label.

## The question

The game's wood stove (a Drolet Escape 1800, `wood_stove` in
`data/machines/home.ron`) took the oxygen its fire burns out of a sealed ship
home's air, about 107 kg a day at full against about 2,900 kg in the home, with
nothing to replace it (docs/PRIORITIES.md, "a wood stove does not balance in a
sealed hull"). In real building practice, where does a solid-fuel stove in a
tight dwelling get its combustion air, and what do the US rules and the stove's
maker require?

## Short answer

- **US manufactured homes (built to the federal construction standard in 24
  CFR Part 3280):** the standard lets a solid-fuel stove in only if it comes
  with "a combustion air inlet" that "shall conduct the air directly into the
  fire chamber" (24 CFR 3280.709(g)(1) and (g)(1)(iii)). Solid-fuel stoves are
  exempted from the stricter rule that every other fuel-burning appliance be
  completely sealed off from the home's air (3280.709(d)), but they still need
  that outside air inlet.
- **The Drolet Escape 1800's maker** says the stove is mobile home approved and
  "REQUIRES THE INSTALLATION OF A FRESH AIR KIT, SOLD SEPARATELY", and that wood
  stoves in mobile homes "must have a ducted supply of combustion air from
  outdoors". Its current manual (2024) names the kit for the model on legs as
  AC01211; the 2019 manual named it AC01316.
- **In an ordinary house** the same manual calls room air "the safest and most
  reliable supply of combustion air", says almost all tight new houses leak
  enough to supply it, and notes only that "Some jurisdictions in the United
  States require" outside air. It warns that an outside air duct can be upset
  by wind.
- **The EPA's Burn Wise programme** says nothing quotable about outside air, and
  its site is now an archive.

Our reading, for the game: a sealed ship home is not an ordinary house. The
maker's case for room air rests on a house's natural leakage replacing what the
fire burns, and a sealed home has none. The federal manufactured-home rule and
the stove's maker, for a mobile home, both say to duct outside air into the
firebox, so the game's stove now has its maker's outside air kit fitted, and a
stove without one stays possible as data. Aboard, "outside" is the ship's air
beyond the home.

## Findings

**1. Solid-fuel stoves in manufactured homes must have a combustion air inlet
that leads outside air straight into the fire.** 24 CFR 3280.709, "Installation
of appliances", read in the eCFR at
https://www.ecfr.gov/current/title-24/subtitle-B/chapter-XX/part-3280/subpart-H/section-3280.709
(fetched through the eCFR's own renderer interface, because the page itself
answered a script with a redirect). The eCFR listed title 24 as "up to date as
of" 2026-10-16, last amended 2026-08-20; the section's own history ends "89 FR
75755, Sept. 16, 2024".

> "(g) Solid fuel burning fireplaces and fireplace stoves listed for residential
> use may be installed in manufactured homes provided they and their installation
> conform to paragraphs (g)(1)(i) through (vii) of this section."

> "(1) A solid fuel-burning fireplace or fireplace stove shall be equipped with
> integral door(s) or shutter(s) designed to close the fireplace or fireplace
> stove fire chamber opening and shall include complete means for venting through
> the roof, a combustion air inlet, a hearth extension, and means to securely
> attach the fireplace or the fireplace stove to the manufactured home structure."

> "(ii) A fireplace or fireplace stove, air intake assembly, hearth extension and
> the chimney shall be installed in accordance with the terms of their listings
> and their manufacturer's instructions."

> "(iii) The combustion air inlet shall conduct the air directly into the fire
> chamber and shall be designed to prevent material from the hearth dropping onto
> the area beneath the manufactured home."

> "(iv) The fireplace or fireplace stove shall not be installed in a sleeping
> room."

**2. Solid-fuel stoves are exempt from the rule that seals other appliances off
completely.** Same section, paragraph (d):

> "(d) All fuel-burning appliances, except ranges, ovens, illuminating
> appliances, clothes dryers, solid fuel-burning fireplaces and solid
> fuel-burning fireplace stoves, shall be installed to provide for the complete
> separation of the combustion system from the interior atmosphere of the
> manufactured home."

So a furnace or water heater in a manufactured home must be a sealed
(direct-vent) appliance or sit in a sealed enclosure, and a wood stove need not
be, though finding 1 still gives it its own outside air. Our reading of why:
a stove's door opens to load wood, so it cannot be sealed off completely.

**3. Every fuel-burning heater in a manufactured home vents outside, and its air
intake joints must hold.** 24 CFR 3280.707, "Heat producing appliances",
https://www.ecfr.gov/current/title-24/subtitle-B/chapter-XX/part-3280/subpart-H/section-3280.707
(history ends "89 FR 75754, Sept. 16, 2024"):

> "(b) Fuel-burning heat-producing appliances and refrigeration appliances,
> except ranges and ovens, shall be of the vented type and vented to the
> outside."

And 24 CFR 3280.710, "Venting, ventilation and combustion air",
https://www.ecfr.gov/current/title-24/subtitle-B/chapter-XX/part-3280/subpart-H/section-3280.710
(history ends "86 FR 2523, Jan. 12, 2021"):

> "(3) Every joint of a vent, vent connector, exhaust duct and combustion air
> intake shall be secure and in alignment."

**4. The Escape 1800 is mobile home approved only with its fresh air kit.**
Stove Builder International, *Installation and Operation Manual, Escape 1800
(DB03102, DB03105, DB03111 & DB03112 models)*, document 45930A, printed
20-03-2019 (20 March 2019),
https://www.drolet.ca/file/45930a_20-03-2019.pdf. The model the game copies is
listed as "DB03105 Escape 1800 - On black legs - black door". Its features table
reads "Approved for mobile home installation Yes", and section 6.1, "Summary of
Installation Cautions and Warnings", says:

> "MAY BE INSTALLED IN A MOBILE HOME"

> "THIS MOBILE HOME APPROVED STOVE REQUIRES THE INSTALLATION OF A FRESH AIR KIT,
> SOLD SEPARATELY."

and, for any installation:

> "IF REQUIRED, A SUPPLY OF COMBUSTION AIR SHALL BE PROVIDED TO THE ROOM OR
> SPACE."

Section 8.5.1, "Combustion Air Supply in Mobile Homes":

> "Wood stoves installed in mobile homes must have a ducted supply of combustion
> air from outdoors. This air supply should be routed down through the house
> floor into the vented crawl space under the mobile home. The air supply duct
> should be non-combustible aluminum flex duct with a screened weatherhood on the
> outside end."

> "Where a mobile home has been converted to a standard house by mounting it on a
> permanent basement foundation, the supply of outdoor air is not required."

Appendix 1, "Installing the Optional Fresh Air Intake Kit":

> "This mobile home approved stove requires the installation of a fresh air
> intake kit (A) and an insulated fresh air intake pipe (B), sold separately."

Its parts list for the models on legs: "AC01316 5"Ø FRESH AIR INTAKE KIT FOR
WOOD STOVE ON LEGS" (the pedestal models' kit is "AC01336").

**5. The current manual keeps the requirement and names a different kit
number.** Stove Builder International, *Wood Stove Owner's Manual, Part 2 of 2,
Escape 1800 Installation and Operation Requirements*, document 46240_IA, dated
2024-02-29, linked from Drolet's page for the DB03105 at
https://www.drolet.ca/file_/7/3/e/3/73e314e6114599a95078dbde8ff4f79786dd916a_46240_IA_2024_02_29.pdf.
Section 6.3, "Optional Fresh Air Intake Kit Installation":

> "This mobile home approved stove requires the installation of a fresh air
> intake kit (A) and an insulated fresh air intake pipe (HVAC type, must meet ULC
> S110 or UL 181 class 0 or class 1) (B), sold separately."

Its parts list for the model on legs: "AC01211 5"Ø FRESH AIR INTAKE KIT FOR
WOOD STOVE ON LEGS". The two manuals disagree on the kit's part number (AC01316
in 2019, AC01211 in 2024); the newer manual is the one Drolet's page links, so
we take AC01211 as the current one. The kit is "optional" in the section's title
because it is required only for a mobile home.

**6. Drolet's product page says the same.** Drolet, "Escape 1800 Wood Stove"
(DB03105), https://www.drolet.ca/us/en/wood-stoves/db03105, read 2026-10-20,
where the model is marked "Discontinued": "Approved for a mobile home
installation: Yes, with fresh air intake".

**7. For an ordinary house, the maker prefers room air.** Manual 45930A (finding
4), section 8.5, "Supply of Combustion Air":

> "In Canada, wood stoves are not required to have a supply of combustion air
> from outdoors (except in mobile homes) because research has shown that these
> supplies do not give protection against house depressurization and may fail to
> supply combustion air during windy weather."

Section 8.5.2, "Air Supply in Conventional Houses":

> "The safest and most reliable supply of combustion air for your wood stove is
> from the room in which it is installed. Room air is already preheated so it
> will not chill the fire, and its availability is not affected by wind pressures
> on the house. Contrary to commonly expressed concerns, almost all tightly sealed
> new houses have enough natural leakage to provide the small amount of air needed
> by the stove."

> "Some jurisdictions in the United States require that wood stoves have a supply
> of combustion air from outdoors."

> "In some windy conditions, negative pressure at the duct weatherhood outside the
> house wall may draw hot exhaust gases from the stove backwards through the duct
> to outdoors."

**8. The manual points to NFPA 211 for the US.** Manual 45930A, section 6.2:

> "In the USA, the ANSI NFPA 211 Standard for Chimneys, Fireplaces, Vents and
> Solid Fuel-Burning Appliances and the ANSI NFPA 70 National Electrical Code are
> to be followed in the absence of local code requirements."

NFPA 211's own text was not read (see below).

**9. The EPA's Burn Wise programme has nothing on outside air, and is archived.**
US EPA, "Burn Wise", https://www.epa.gov/burnwise, read 2026-10-20 through a
fetching tool that reported the page as last updated 3 September 2026: "This
website serves as a historical record; EPA is no longer updating program content
as of October 30, 2025." Its page on burning tips
(https://www.epa.gov/burnwise/burning-tips-and-tricks) returned an error that
day. A web search for Burn Wise guidance on outside combustion air found none.
This is a negative finding: the EPA's consumer wood-stove programme does not
address where a stove's combustion air should come from.

## What is still unknown, and what would settle it

- **NFPA 211's rule on combustion air for solid-fuel appliances.** The manual
  defers to it in the US. Settled by reading the current edition on NFPA's free
  read-only access (it needs a free account) or a library copy.
- **Part 1 of Drolet's current owner's manual** (document 46231_UA, 2024-09-10,
  linked from the same product page). It is 30 pages, but its text could not be
  extracted, and it likely holds the current wording of the general combustion
  air section quoted above from the 2019 manual. Settled by reading the PDF by
  eye.
- **How much air the kit carries and how much of the fire's air it supplies.**
  Neither manual gives a flow. A stove with a kit still draws some room air past
  its door gasket and whenever the door opens to load, so "all of the fire's air
  from outside" is our simplification. Settled by the kit's own instructions
  (AC01211), which were not found online, or a test report.
- **The International Residential Code's rule for stoves in site-built houses.**
  Not researched here; finding 7 says only that some US jurisdictions require
  outside air.
- **What a real spacecraft or ship habitat does with a fire's air and exhaust.**
  Not researched: no source was looked for, because no crewed vehicle burns
  wood. The game's answer is a game rule (below).

## Sources that could not be reached

- https://www.ecfr.gov/current/title-24/subtitle-B/chapter-XX/part-3280/subpart-H/section-3280.709
  answered scripted access with a redirect to an "unblock" page; the same text
  was read through
  https://www.ecfr.gov/api/renderer/v1/content/enhanced/current/title-24?part=3280&section=3280.709
  (and the same for 3280.707 and 3280.710).
- https://www.epa.gov/burnwise/burning-tips-and-tricks returned HTTP 502.
- The 46231_UA manual's text (above) could not be extracted.
- NFPA 211 is behind NFPA's account wall; not attempted.

## What it means for this project

**Certain:**

- In a US manufactured home, a wood stove must be a listed fireplace stove with
  its own combustion air inlet leading outside air directly into the fire
  chamber, installed with its air intake assembly per its listing (24 CFR
  3280.709(g)).
- The Drolet Escape 1800, the stove the game copies, is approved for a mobile
  home only with its fresh air intake kit and an insulated intake duct, sold
  separately; for the model on legs the current manual names kit AC01211.
- For an ordinary site-built house the maker recommends room air, relying on
  the house's natural leakage to replace what the fire burns.

**Judgement calls (ours, not the sources'):**

- A sealed ship home is the extreme of a tight home: its air leaks only
  overboard, and nothing comes in to replace what a fire burns. The maker's
  argument for room air does not hold there, and the manufactured-home rule's
  answer does. So the game's shipped stove has its outside air kit fitted
  (`combustion_air: Outside` in `burns_fuel`), and its fire takes none of the
  home's oxygen.
- Aboard, "outside" the home is the ship's air beyond it, which the ship's life
  support keeps at the standard a home starts at and the game does not model
  (`src/ship/ship_space.rs`). The game does not invent an air system for it: the
  oxygen the stove burns is metered against the home in the ship's supply
  ledger (`ship_power::OXYGEN`, kilograms), in both Ship life support modes,
  because those modes decide who powers the home and the ship's air is the
  ship's in either.
- A stove with no kit (`combustion_air: Room`) stays possible as data and burns
  the home's own oxygen, so the danger the Library's heating guide teaches is
  still real in the game.
- Not modelled, and stated as such in the machine's data and the Library
  guides: the excess air a real stove pulls through its fire, where the flue's
  exhaust goes aboard, a duct upset by wind, and room air drawn past the door.

This is a reading of public sources, not legal advice, and not installation
instructions. Install a real stove by its own manual and certification label,
and ask your local building official.
