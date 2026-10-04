# Pipe marking standards: findings

**Research date: 2026-10-04.** Every source below was read on 2026-10-04. This
is a reading of public sources, done so the game can mark its pipes the way
real ones are marked. **It is not legal, safety or engineering advice**, and
nobody who wrote it is a piping engineer, a safety officer or a lawyer. Before
marking a real pipe, read the standard that applies where you are, in its
current edition, and ask a professional.

**The question.** Are there international standard markings for pipes (colours,
bands, labels, arrows), or would the game have to invent its own to cover the
full spectrum of materials? The operator asked it on 2026-10-04, wanting pipes
in the game to show their real material, paint included, and to be
identifiable "with colour coordinate and labels and stripes".

Standards examined: ASME A13.1 (United States), ISO 14726 (ships), ISO 20560-1
(international, general), BS 1710 (United Kingdom), DIN 2403 (Germany), and the
spacecraft and NASA practice for fluid lines (MIL-STD-1247, the ISS crew
integration standard SSP 50005, NASA-STD-3001, NASA ground standards). Two
neighbours were added because they answer the "material" half of the question:
the copper tube colour code and the purple marking of non-drinking water in a
US plumbing code.

---

## Short answer

- **Yes, there are standards, and we should not invent our own.** At least
  seven published schemes exist, two of them international (ISO 14726 for
  ships, ISO 20560-1 for piping in general), and both ISO ones explicitly
  allow use on ships.
- **Every one of them marks the CONTENT, never the pipe's MATERIAL.** They say
  what flows inside (water, steam, fuel, oxygen), not whether the wall is
  copper, steel or plastic. So there is nothing to invent for materials: a
  copper pipe looks like copper. Material identification, where it exists, is
  the manufacturer's printed marking on the pipe itself (the copper tube
  type stripes K green, L blue, M red, DWV yellow are an example, F31).
- **Colour alone means different things in different schemes.** Blue is fresh
  water on a ship (ISO 14726), compressed air in US plants (ASME A13.1) and in
  ISO 20560-1, oxygen in Germany (DIN 2403), and coolant on aerospace lines
  (MIL-STD-1247). Red is fire fighting nearly everywhere, but steam in DIN 2403
  and fuel on aircraft. That is why **every scheme also requires a written
  name**, and why NASA's current spacecraft standard (2026) requires "an
  additional cue" whenever colour carries critical meaning (F26).
- **The "full spectrum" is covered by text, not by more colours.** Each scheme
  has a handful of colour groups (six to twelve) and then writes the content's
  name on the pipe. ISO 14726 adds a second level: a band of an "additional colour"
  between two bands of the main colour (potable water is blue-green-blue,
  condensate blue-yellow-blue), which gives well over a hundred distinct
  codes.
- **All of them let the real pipe show between the markers.** Colour may be
  applied as bands or labels at intervals instead of over the whole length
  (ASME A13.1, BS 1710, MIL-STD-101C, which even calls bands "preferred").
  Pipes that need no warning colour "may be painted to match surroundings ...
  aluminum, black, or remain unpainted" (MIL-STD-101C, F8).
- **Spacecraft practice adds two rules the plant standards do not stress:**
  lines are labelled "to allow for positive identification", and connectors
  for different contents have different shapes so they cannot be cross-mated
  (ISS, F25; NASA-STD-3001, F27).
- **Recommendation (a judgement call, see the end):** aboard the ship, use
  **ISO 14726 colours** (main colour bands, plus additional-colour triples for
  specific media); use **ISO 20560-1's non-colour elements** for the label
  (content name, flow arrow, GHS hazard pictograms); place markers by the
  common rule all schemes share (at valves, branches, bends and wall
  penetrations, and at intervals); key connectors per content as NASA does.
  Render the pipe's **real material and paint everywhere between the
  markers**. The game's current pipe colours conflict with most schemes on
  red, yellow and violet, and the code comment that cites ASME A13.1 does not
  match it (see "What it means for this project").
- **Every standard text is copyrighted except the US government ones.** We
  restate facts (which colour means what) in our own words and data, quote at
  most short phrases, and never copy tables or figures wholesale. MIL-STD,
  NASA and the German TRGS rule are public documents and can be quoted more
  freely.

---

## How the sources were handled

- **Primary first.** Each standards body's own catalogue page was read for
  scope, edition and date. The texts themselves are sold, so the colour
  schemes came from, in order of preference: free official preview pages
  (ISO's sample pages served by iTeh, ASME's own front-matter PDF);
  government documents that restate or adopt the standard (NASA, US
  Department of Defense, OSHA, the German TRGS 201); an extract reproduced
  with the publisher's permission (the Water Regs UK leaflet for BS 1710); and
  last, vendor pages, which are marked as secondary every time they are used.
- **Text extraction.** PDFs were turned into text with pdftotext. ISO 14726's
  Table 3 is a two-column table, and the "layout" extraction misaligned its
  rows by one; the content-stream ("raw") extraction was used instead. Two
  checks confirm the raw reading: it gives potable water blue-green-blue and
  condensate blue-yellow-blue, exactly as DIN's own summary of the standard
  says (F13), and in the "air and sounding pipes" block every additional
  colour matches the main colour of the medium named beside it (maroon-blue-
  maroon beside "Fresh water", maroon-brown-maroon beside "Fuel", and so on).
  Even so, any additional-colour code taken from this document should be
  checked against the printed standard before it ships as game data.
- **Quotations are exact**, including the sources' own typos, which are marked
  [sic]. Where extraction turned a hyphen into "---" it is written as a hyphen.
- "Computed" or "restated" marks our own summary of a source, not its words.

---

## A. United States: ASME A13.1 and the federal documents built on it

**F1. ASME's catalogue page for A13.1.**
[asme.org/codes-standards/find-codes-standards/a13-1-scheme-identification-piping-systems](https://www.asme.org/codes-standards/find-codes-standards/a13-1-scheme-identification-piping-systems),
read 2026-10-04 (the page's product record carries "Updated":
2026-10-04). It describes the standard:

- "A13.1 establishes a common system to assist in identification of fluids
  conveyed in piping and their characteristics. The Standard describes
  requirements for the identification of aboveground piping used in
  industrial, commercial, transmission, distribution, and institutional
  installations, and in buildings used for public assembly. It does not apply
  to electrical conduits."
- The page's data lists five editions: 2007 (R2013), 2015, 2020, 2023 and
  **2026**. The 2026 record is a 19-page PDF, ISBN 9780791877999, listed at
  USD 50. Its exact release date was not shown; a search summary said 15 July
  2026, which we could not confirm from a primary page.

**F2. ASME A13.1-2015 front matter, served by ASME.**
[asme.org/getmedia/.../35817.pdf](https://www.asme.org/getmedia/5778cda2-cda4-438b-93c6-c209afcaa729/35817.pdf),
"Date of Issuance: December 29, 2015". Only the cover, notices and contents
are included. The contents list "Table 2 Designation of Colors" and "Figure 1
GHS Pictograms", and the notice reads "No part of this document may be
reproduced in any form ... without the prior written permission of the
publisher."

**F3. ANSI's description of what changed in 2015, 2020 and 2023.**
Brad Kelechava, "Identification of Piping Systems Through ASME A13.1-2023",
[blog.ansi.org](https://blog.ansi.org/ansi/identification-piping-systems-asme-a13-1-2023/),
published 6 December 2023, modified 28 October 2025. ANSI is the US national
standards body.

- On scope: "it does not apply to buried pipelines or electrical conduits."
- On 2015: "the 2015 edition of ASME A13.1 saw the addition of GHS pictograms
  and a definition for oxidizing."
- On 2023: "Both the Color section and Table 4.2-1, “Designation of Colors,”
  were revised." Also "ASME A13.1 was changed from a periodic-maintenance to a
  continuous-maintenance Standard", and new figures were added for flow to the
  right and flow in both directions.
- On 2020: "Former paragraph 3.5 ... was revised in its entirety and replaced
  by 4.5, “Abandoned Piping.”"
- **Consequence:** any colour table from before 2023 (most vendor charts) may
  be out of date. We did not read the 2023 or 2026 table itself.

**F4. A NASA centre standard that adopts A13.1 and restates its colours.**
GSFC-STD-8006, *Safety Standard for Ground Piping Systems Color Coding and
Identification*, NASA Goddard Space Flight Center, approved 2017-11-17,
revalidation date 2022-11-17,
[EverySpec listing](https://everyspec.com/NASA/NASA-GSFC/GSFC-STD/GSFC-STD-8006_55882/)
(status "Active"). "APPROVED FOR PUBLIC RELEASE ... DISTRIBUTION IS
UNLIMITED."

- "ASME A13.1, Scheme for the Identification of Piping Systems, forms the
  basis for this standard, and is incorporated by reference."
- Its warning colours (4.1.1), restated: fire quenching fluids, safety red
  with white legend; toxic and corrosive, safety orange, black legend;
  flammable and oxidizing, safety yellow, black legend; combustible, safety
  brown, white legend; "Potable, Cooling, Boiler Feed, and Other Water",
  safety green, white legend; compressed air, safety blue, white legend.
  GSFC also assigns anesthetic and harmful fluids to safety purple and
  physically dangerous fluids (cryogens, nitrogen, steam) to safety gray.
- On paint and bare pipe (4.2.1): "Piping systems that do not require warning
  colors may be painted to match surroundings, if not in conflict with other
  color designations in this standard, or such systems may be painted
  aluminum, black, or remain unpainted."
- On placement (4.2.2): bands and legends "shall be applied not less than 20
  feet apart on straight pipe runs inside buildings and congested exterior
  areas, and 50 feet apart on long exterior runs" (the wording says "not less
  than"; the intent reads as a maximum spacing), also "immediately adjacent to
  and upstream of all operating accessories such as valves", "where branch
  lines join the system, at changes in direction, where the system passes
  underground or through walls, at flanges".
- On spacecraft (1.2): "This standard is not applicable to ... pipelines in
  missiles, spacecraft, other airborne equipment, or storage vessels."

**F5. The A13.1 categories and two sentences of its colour clause, as quoted
on a vendor page (secondary).**
Brimar, "ASME Guide",
[pipemarker.com/asme-guide](https://www.pipemarker.com/asme-guide), undated,
headed "Latest Revision ANSI / ASME A13.1 2015". The category names are:
"Fire Quenching Fluids", "Toxic & Corrosive Fluids", "Flammable Fluids",
"Combustible Fluids", "Potable, Cooling, Boiler feed, & other Water",
"Compressed Air", and four rows "Defined by the User" (their colours are
images, not text). The page quotes the standard's paragraph 4.2, which
matches the post-2020 numbering:

- "its use shall be used in combination with a legend. Color may be used in
  continuous, total-length coverage or in intermittent displays."

So A13.1 itself allows colour as intermittent labels with bare or painted
pipe between them. The colours of the four user-defined slots were not read
from any primary source.

**F6. US federal regulation incorporates an old edition.**
29 CFR 1910.261, Pulp, paper, and paperboard mills,
[osha.gov](https://www.osha.gov/laws-regs/regulations/standardnumber/1910/1910.261),
read 2026-10-04:

- "1910.261(a)(3)(ii) Scheme for the Identification of Piping Systems, A13.1 -
  1956."
- "All chlorine, caustic, and acid lines shall be marked for positive
  identification, in accordance with American National Standard A13.1 -
  1967."

This is the only federal text found that requires A13.1, and only for that
industry. No general federal requirement to follow A13.1 was found (not
searched exhaustively).

**F7. The Department of Defense colour code for ground pipelines, which
points aircraft and space lines elsewhere.**
MIL-STD-101C, *Color Code for Pipelines and for Compressed Gas Cylinders*,
26 August 2014, superseding MIL-STD-101B (1970),
[WBDG copy](https://www.wbdg.org/FFC/FEDMIL/milstd101.pdf), "DISTRIBUTION
STATEMENT A. Approved for public release; distribution is unlimited."

- Scheme: six hazard classes, each with a colour (4.2): yellow flammable,
  brown toxic and poisonous, blue anesthetics and harmful, green oxidizing,
  gray physically dangerous, red fire protection. Applied as a "primary color
  warning" band and a "secondary color warning" arrow showing flow direction
  (5.1.3, 5.1.4).
- Drinking water: "Water-piping systems containing water suitable for human
  consumption and installed for this purpose shall be painted White, No.
  17875 throughout" (4.3).
- Pointer (1.2): "The identification of pipe lines for aircraft, missiles,
  and space vehicles is covered in MIL-STD-1247."

**F8. The same standard on bands versus paint, and on bare pipe.**
MIL-STD-101C, 5.1.2 and 5.1.3.1:

- "Piping systems which do not require warning colors may be painted to match
  surrounding, if not in conflict with other color designations in this
  standard or such systems be painted aluminum, black, or remain unpainted."
- Whole-system painting is allowed, "However, the use of color bands is
  preferred because they will indicate dangerous systems to color-blind
  personnel."
- Exact identification is "mandatory and shall be made only by means of
  titles lettered in black or white" (5.1.1).

---

## B. Ships: ISO 14726

**F9. ISO's catalogue page.**
[iso.org/standard/44744.html](https://www.iso.org/standard/44744.html), read
2026-10-04. ISO 14726:2008, *Ships and marine technology - Identification
colours for the content of piping systems*, edition 1, published 2008-05, 13
pages, committee ISO/TC 8/SC 3. "This publication was last reviewed and
confirmed in 2024. Therefore this version remains current." It replaced ISO
14726-1:1999 (main colours) and ISO 14726-2:2002 (additional colours).

**F10. Scope and definitions (free sample pages).**
ISO 14726:2008, first edition 2008-05-01, sample served by iTeh,
[cdn.standards.iteh.ai/.../ISO-14726-2008.pdf](https://cdn.standards.iteh.ai/samples/44744/ed36a8bbb25d40e1a011ea894b94b130/ISO-14726-2008.pdf)
(pages up to the end of clause 4):

- "This International Standard specifies main colours and additional colours
  for identifying piping systems in accordance with the content or function
  on board ships and marine structures."
- "This International Standard does not apply to piping systems for medical
  gases, industrial gases and cargo."
- "This International Standard can also be used for land installations."
- Main colour: "colour used to indicate a group of similar media". Additional
  colour: "colour used in combination with the main colour to indicate a
  specific medium".
- On exact shades: "colours of a similar shade and tone may also be used for
  marking pipes". Table 1 gives each main colour as CIE 1931 chromaticity
  corner points and a luminance factor.

**F11. The twelve main colours (Table 2, restated).**
Black: waste media (the footnote's examples are black water, grey water, waste
oil, exhaust gas). Blue: fresh water. Brown: fuel. Green: sea water (for
sea-river ships "all outside waters"). Grey: non-flammable gases. Maroon: air
and sounding pipes. Orange: oils other than fuels. Silver: steam. Red: fire
fighting. Violet: acids, alkalis. White: air in ventilation systems. Yellow:
flammable gases.

**F12. Additional colours (Table 3, a few examples, restated from the raw
extraction; check against the printed standard).**
Every specific medium is a triple: main colour, additional colour, main
colour. Examples: potable water blue-green-blue; condensate blue-yellow-blue;
chilled water blue-white-blue; oxygen grey-blue-grey; nitrogen
grey-green-grey; compressed air, low pressure, grey-orange-grey; compressed
air, high pressure, grey-red-grey; breathing air grey-white-grey (a footnote
says this marking "is used in submarines for distribution systems of
breathing air from cylinders"); hydraulic fluid orange-grey-orange; sprinkler
water red-orange-red; hydrogen yellow-blue-yellow. Many triples are left
unassigned, so a ship (or a game) has spare codes.

**F13. DIN's summary of the same standard confirms the band triple.**
DIN ISO 14726:2010-10,
[din.de/en/wdc-beuth:din21:133855446](https://www.din.de/en/wdc-beuth:din21:133855446),
read 2026-10-04:

- "Additional colours are necessary in those cases where a distinction has to
  be made between pipes marked with the same main colour, for example blue
  for water, but carrying different types of medium, for example a pipe
  carrying potable water (marking: blue-green-blue) and another pipe carrying
  condensate (marking: blue-yellow-blue)."

Clause 5 "Design" (band widths, order, legends, arrows) and Annex B
("Standard colours and equivalent colour codes") are not in the free sample
and were not read.

---

## C. International, general: ISO 20560-1

**F14. ISO's catalogue page for the current edition.**
[iso.org/standard/86049.html](https://www.iso.org/standard/86049.html), read
2026-10-04. ISO 20560-1:2024, *Safety information for the content of piping
systems and tanks - Part 1: Piping systems*, edition 2, published 2024-06, 25
pages, committee ISO/TC 145/SC 2. It replaced ISO 20560-1:2020. The abstract
includes: "This document does not cover piping that is buried." and "This
document can also be used for marine structures and ships."

**F15. Its scheme, from the free sample of the 2020 edition.**
ISO 20560-1:2020, first edition 2020-09, sample served by iTeh,
[cdn.standards.iteh.ai/.../ISO-20560-1-2020.pdf](https://cdn.standards.iteh.ai/samples/71570/55d7518886df4ad1819aef67127eacdc/ISO-20560-1-2020.pdf).
This is the superseded edition; the 2024 changes were not read.

- "A safety information system for piping shall consist of four key
  elements: 1) colour coding to identify the nature of the content in the
  piping; 2) content name; 3) flow direction indicators; 4) when applicable,
  warning signs, GHS pictograms or both." (5.1)
- Colours (Tables 1 and 3, restated): yellow is the safety colour for
  hazardous substances; basic identification colours are grey for "Gases in
  either gaseous or liquefied condition", black for "Liquids and fixed
  materials (powder, granulates)", orange for acids, violet for "Alkalis
  (leaches)", red for firefighting medium, green for water, blue for air.
- "Where there is no need to further differentiate hazardous substances, the
  safety colour yellow shall be used alone, without the addition of a basic
  identification colour." (5.2)
- Label text: "Alternatively, the content name shall be the contrast colour
  black on a white background." (5.3) Sans serif, upper and lower case.
- Arrows: "The direction of flow shall be indicated with a single headed
  arrow (see Figure 2) or, where applicable (e.g. ring main), with a
  double-headed arrow (see Figure 3)." (5.4)
- Its colours' origin (Table 2, Note 2): "All colours except yellow and red
  are amended from ISO 14726. Yellow and red are safety sign colours from ISO
  3864-4."
- Its origin in national schemes: "Many different countries' national pipe
  marking standards were reviewed during the development of this document."
- It carries "Annex E (informative) Maritime piping systems", which is not in
  the sample. How it reconciles blue-for-air here with blue-for-fresh-water in
  ISO 14726 is unknown.

---

## D. United Kingdom: BS 1710

**F16. BSI's catalogue page.**
[knowledge.bsigroup.com/products/specification-for-identification-of-pipelines-and-services](https://knowledge.bsigroup.com/products/specification-for-identification-of-pipelines-and-services),
read 2026-10-04. BS 1710:2014, published 31 December 2014, validity
"current", ISBN 978 0 580 83113 3, committee PSE/4. The page shows the
standard's opening clauses:

- "This British Standard specifies the colours and supplementary information
  for the identification of pipes conveying fluids in above ground and below
  ground installations. It also includes ducts for ventilation and conduits
  used for carrying electrical services."
- Two methods: "basic identification colours only; and" "basic
  identification colours and code indications and/or code colours."
- "This British Standard does not include identification of fluid services on
  ships." NOTE 3 points to BS ISO 14726.
- "This British Standard supersedes BS 1710:1984, which is withdrawn."

**F17. Extract reproduced with BSI's permission: water services.**
Water Regs UK, "Pipe identification", Version 1.1, April 2021,
[waterregsuk.co.uk/.../pipe_identification_bs_1710.pdf](https://www.waterregsuk.co.uk/downloads/publications/info_leaflets/pipe_identification_bs_1710.pdf).
Its tables are "based on table 1 in BS 1710:2014" and carry the note
"Permission to reproduce extracts from BS 1710:2014 is granted by BSI
Standards Limited (BSI)."

- Restated: water is green (BS 4800 colour 12 D 45); the code and safety
  colours are red for fire (04 E 53), auxiliary blue for water from a public
  supply (18 E 53), flint grey for water from any other source (00 A 09), and
  yellow for warning (08 E 51). A marker is basic colour, code colour, basic
  colour.
- "Basic identification colours can be applied over the whole length of a
  service or as bands."
- "Decorative or protective coverings shall be of a contrasting colour to the
  basic identification colour."
- Placement: at junctions, both sides of a valve and of a wall penetration,
  visible in every section; "Concealed services shall be marked at regular
  intervals of not more than 500mm."

**F18. The full set of BS 1710 basic colours: two vendor pages that
disagree (secondary).**

- Label Source, "BS 1710 - The British Way of Marking Pipes",
  [labelsource.co.uk](https://www.labelsource.co.uk/news/post/bs-1710-the-british-way-of-marking-pipelines),
  12 January 2018: water green 12 D 45; steam silver-grey 10 A 03; "Oils
  (mineral,vegetable or animal)" brown 06 C 39; "Gases (in either gas or
  liquid phase - except air)" yellow ochre 08 C 35; acids and alkalis violet
  22 C 37; air light blue 20 E 51; "Other Liquids" black 00 E 53; electrical
  conduits and ducts "apricot" (orange) 06 E 51.
- Silver Fox, "BS 1710 Pipe Colour Codes: UK Guide",
  [silverfox.co.uk](https://silverfox.co.uk/blogs/news/bs-1710-pipe-colour-codes-uk-guide),
  undated, as summarised by our fetch tool: the same codes, but silver grey
  10 A 03 for "Other liquids", grey 10 A 05 for steam, and black for
  "Drainage, electrical services, other services".
- **Judgement:** Label Source is more credible here. Its water entry matches
  the BSI-permitted extract (F17), its list is internally consistent with the
  BS 4800 codes, and the Silver Fox text came to us through a summarising
  tool rather than as raw text. Neither is the standard; settle it by reading
  BS 1710:2014 itself.

---

## E. Germany: DIN 2403 and the federal rule that restates it

**F19. DIN's catalogue page for the current edition.**
DIN 2403:2025-12, *Kennzeichnung von Rohrleitungen nach dem Durchflussstoff*
(Identification of pipelines according to the fluid conveyed), 17 pages,
[din.de/en/wdc-beuth:din21:395266777](https://www.din.de/en/wdc-beuth:din21:395266777),
read 2026-10-04; DIN Media lists the 2018-10 edition as withdrawn and replaced
by 2025-12, and the 2014-06 edition as replaced by 2018-10.

- "This standard is applicable for the identification of pipelines in
  aboveground installations according to the fluid conveyed. This document
  may also be applied to the marking of flexible lines."
- It excludes "The identification of piping systems on ships and marine
  installations as specified in DIN ISO 14276 [sic]" (the standard meant is
  ISO 14726), ventilation in non-residential buildings and medical gases.
- DIN Media's summary of the 2024-12 draft (paraphrased by our fetch tool,
  not quoted) lists the changes as flexible lines, chlorine gas examples and
  editorial revision. Whether the colour groups changed in 2025 is unknown.

**F20. The groups and colours, from a German federal technical rule.**
TRGS 201, *Einstufung und Kennzeichnung bei Tätigkeiten mit Gefahrstoffen*,
"Ausgabe Februar 2017", GMBl 2017 p. 218, last amended GMBl 2018 p. 234,
Annex 3,
[vorschriften.bgn-branchenwissen.de/daten/tr/trgs201/anh3.htm](https://vorschriften.bgn-branchenwissen.de/daten/tr/trgs201/anh3.htm)
(mirror of the rule published by the Federal Ministry of Labour). Its Table 2
"beispielhaft" (by way of example), restated with group numbers: water (1)
green, white text; steam (2) **red**, white text; air (3) grey, black text;
flammable gases (4) yellow with red additional colour; non-flammable gases
(5) yellow with black; acids (6) orange; alkalis (7) violet; flammable liquids
and solids (8) brown with red; non-flammable liquids and solids (9) brown with
black; oxygen (0) **blue**.

- "Gruppenfarbe und Zusatzfarbe bilden die Basis der Kennzeichnung von
  Durchflussstoffen in Rohrleitungen. Der Durchflussstoff selber sowie die
  Durchflussrichtung sind ebenfalls anzugeben." (Group colour and additional
  colour are the basis; the fluid itself and the flow direction must also be
  given.)
- The annex cites "DIN 2403:2014-06", so this is the 2014 grouping.
- Section 4.5.3 (3): "Auf die Verwendung des Piktogramms GHS04 "Gasflasche"
  sollte verzichtet werden." (The gas-cylinder pictogram should not be used on
  pipelines.) GSFC-STD-8006 says the same in English (F4).

**F21. A trade magazine on the German practice (secondary).**
"Kennzeichnungen an Rohrleitungen", *SBZ Monteur* 2009/10, pp. 32-33,
[sbz-monteur.de](https://www.sbz-monteur.de/sites/default/files/sbzm_pdf/file_259827.pdf):
lines are marked at most every ten metres ("in einem Abstand von maximal zehn
Metern") and at the start and end, at branches, wall penetrations and
fittings, by paint and lettering, self-adhesive tape, or signs of metal or
plastic. It also notes that the German drinking water ordinance requires
non-drinking lines to be marked where both kinds are installed.

---

## F. Spacecraft and NASA practice

**F22. The aerospace standard for lines: MIL-STD-1247D.**
*Markings, Functions and Hazard Designations of Hose, Pipe, and Tube Lines for
Aircraft, Missile, and Space Systems*, 29 January 2009, superseding
MIL-STD-1247C (1989),
[EverySpec listing](https://everyspec.com/MIL-STD/MIL-STD-1100-1299/MIL-STD-1247D_21215/)
(status "Inactive", Notice 1 dated July 2014). Cover: "This document is
inactive for new design." "DISTRIBUTION STATEMENT A. Approved for public
release; distribution is unlimited." (The copy we read carried a reseller's
stamp; the document itself is a public US government standard.)

- Scope (1.1): "This standard establishes material labeling requirements for
  identification, function, subfunction, pressures, hazards and direction of
  flow for pipes, hoses and tube lines used in aircraft, missile, space
  systems, and support equipment."
- Scheme: coded by **function**, on a tape that encircles the line, with
  colour stripes on the left, the function in words, and a repeated black
  geometric symbol on the right (5.1.1 to 5.1.3). "Lettering shall be black,
  regardless of the background color."
- Colours (5.1.1.1, restated): fuel red; rocket oxidizer green-gray; rocket
  fuel red-gray; lubrication yellow; hydraulic blue-yellow; pneumatic air
  orange-blue; coolant blue; breathing oxygen green; vacuum gray-orange-gray;
  fire protection brown; de-icing gray; compressed gas orange; electrical
  conduit brown-orange; inerting fluid orange-green; and others.
- Drinking water (4.1 a): "Water piping systems containing water for human
  consumption shall be painted white or identified as directed by the
  procuring activity."
- Hazards are words, black on white or metallic, e.g. "FLAM", "TOXIC" (4.1.3,
  Table II). Flow: "A two-headed arrow will be used to indicate reversible
  flow." (4.1.4)
- Bare steel: "Carbon steel line and other lines requiring a protective finish
  will, in addition, be painted white ... as background for the identification
  group." (4.2.4)
- Placement (5.7.2): "In general, identification need by [sic] placed only at
  intervals to insure that at least one group is visible and recognizable from
  any observation point along the line."

**F23. NASA Kennedy's ground practice (cancelled standard, surviving
summary).** KSC-STD-SF-0004, *Safety Standard for Ground Piping Systems Color
Coding and Identification*:
[standards.nasa.gov/node/896](https://standards.nasa.gov/node/896) lists
version C, document date 08/13/2015, "INACTIVE", cancelled 09/03/2015. The
Rev B PDF there is a scanned image and was not read. KSC's preferred practice
DFE-5, *Ground Piping Systems Color Coding and Identification*,
[extapps.ksc.nasa.gov/.../dfe5.pdf](https://extapps.ksc.nasa.gov/Reliability/Documents/Preferred_Practices/dfe5.pdf),
undated, summarises it: the MIL-STD-101 hazard colours, a flow arrow, a
title and a pressure, and "all fire lines are painted red". It too states the
practice "is not applicable to ... pipelines installed in missiles,
spacecraft airborne equipment, or storage vessels."

**F24. NASA ground support equipment: keyed fittings and colour coding for
propellants.** NASA-STD-5005D w/Change 1, *Standard for the Design and
Fabrication of Ground Support Equipment*, approved 2013-06-14, revalidated
2017-10-05,
[standards.nasa.gov PDF](https://standards.nasa.gov/sites/default/files/standards/NASA/D-w/CHANGE-1/1/nasa-std-5005d_w_chg1_revalidated.pdf)
(a later Change 2 exists per NASA's listing; not read):

- 5.2.3 a, to keep hypergolic fuel and oxidizer apart: "(1) Color coding and
  marking of hypergol fuels and oxidizer servicing equipment." and "(2)
  Uniquely sizing or using a mechanically different type of connection for
  each commodity where close proximity creates a potential for
  cross-connection."
- 5.9: "Dissimilar fittings/connectors to prevent cross-connecting fluid lines
  or electrical cables."

**F25. The International Space Station: labels and connector shapes, not a
colour code.** SSP 50005E, *International Space Station Flight Crew
Integration Standard (NASA-STD-3000/T)*, Revision E, 30 June 2006,
[EverySpec listing](https://everyspec.com/NASA/NASA-JSC/NASA-SSP-PUBS/SSP_50005E_29661/):

- 12.3.1.3 C (5): "Electrical cables, fluid lines, and other subsystem
  protective shields shall be labeled to allow for positive identification."
- 11.10.3.5 A: "Connectors which are of different shapes and physically
  incompatible shall be used when lines differ in content (i.e., different
  voltages, liquids, gases, etc.)." Both halves of a connection carry a code
  "unique to that connection" (11.10.3.5 E).
- On colour in general (9.5.3.2 I): "No more than nine colors, including white
  and black, shall be used in a coding system." Colours "will not be used as a
  primary identification medium if the spectral characteristics of ambient
  light during the mission, or the operator's adaptation to that light,
  vary".
- No fluid-line colour scheme was found in this standard. The ISS practice
  for line colours, if any, is still unknown (see the open questions).

**F26. NASA's human-system standard, current edition: colour must never
stand alone.** NASA-STD-3001, Volume 2, Revision F, *Human Factors,
Habitability, and Environmental Health*, approved **2026-07-14**,
[standards.nasa.gov PDF](https://standards.nasa.gov/system/files/tmp/NASA-STD-3001%20Vol%202%20Rev%20F.pdf),
listed "ACTIVE" at [standards.nasa.gov/node/237](https://standards.nasa.gov/node/237).

- [V2 10045]: "The system shall provide an additional cue when color is
  issued [sic] to convey meaning for critical information or for a critical
  task." Rationale: "Redundant coding is required to accommodate the
  variability in people's capability to see color under different lighting
  conditions".

**F27. The same document on lines and connectors.**

- [V2 9032]: "Cable, gas and fluid lines, and electrical umbilical connectors
  shall prevent potential mismating and damage associated with mating or
  demating tasks."
- [V2 12037]: "Connectors shall have physical features to preclude incorrect
  mating and mismating." Its rationale lists "color coding, different size
  connectors, connector keying, and tactile feedback".

---

## G. Material identification (the half the marking standards do not cover)

**F28. None of the content standards codes the pipe material.** Restated from
F1 to F27: A13.1, ISO 14726, ISO 20560-1, BS 1710, DIN 2403, MIL-STD-101C and
MIL-STD-1247D all assign colour by content, hazard or function. None assigns a
colour to copper, steel, plastic or any other wall material.

**F29. Bare, painted or matched pipe is explicitly allowed.** MIL-STD-101C
(F8) and GSFC-STD-8006 (F4): pipes that need no warning colour may match the
surroundings, be aluminium or black, or stay unpainted. A13.1 (F5) and BS 1710
(F17) allow colour as bands or intermittent labels. BS 1710 (F17) requires a
decorative covering to contrast with the marking colour.

**F30. One colour code that IS on the pipe body: non-drinking water in US
plumbing codes.** IAPMO Code Spotlight on the 2015 Uniform Plumbing Code,
section 1503.7,
[forms.iapmo.org](https://forms.iapmo.org/email_marketing/codespotlight/2017/Aug3.htm),
3 August 2017: "ALL ALTERNATE WATER SYSTEMS SHALL HAVE A PURPLE (Pantone color
No. 512, 522C, or equivalent) background with upper case lettering and shall
be field or factory marked", and "Many manufactures are already supplying
prelettered and colored piping for most applications." So purple pipe is a
case where the manufactured pipe itself carries the content colour.

**F31. Copper tube carries a type colour, which is a material marking, not a
content code.** Copper Development Association, *Copper Tube Handbook*, Table
14.1,
[copper.org](https://copper.org/applications/plumbing/cth/technical-data/tables/cth_table1.php)
(now returns 404; read through the Internet Archive capture of 29 April 2025,
[web.archive.org](https://web.archive.org/web/20250429190810/https://www.copper.org/applications/plumbing/cth/technical-data/tables/cth_table1.php)).
Restated: Type K green, Type L blue, Type M red (all ASTM B 88); DWV yellow
(ASTM B 306); ACR blue (ASTM B 280); medical gas tube (K) green, (L) blue
(ASTM B 819). These colours mark wall thickness and grade. They collide with
content colours (red here is not fire), which is one more reason content
markers must carry words.

**F32. Plastic tube colours are a sales convention, as far as we found
(secondary).** A PEX maker's product pages (for example
[supplyhouse.com, Mr PEX red tubing](https://www.supplyhouse.com/Mr-PEX-1740010-3-4-Red-Potable-PEX-Tubing-100-ft-Coil))
offer the same potable tubing in red, white and blue to tell hot from cold.
Whether any ASTM or ISO product standard fixes those colours was not
researched.

---

## H. Can the text be used?

Restated; this is our reading, not legal advice.

| Source | Owner | What we can do |
|---|---|---|
| ASME A13.1 | ASME, copyrighted ("No part of this document may be reproduced", F2) | Restate facts; quote short phrases; buy the PDF to read it |
| ISO 14726, ISO 20560-1 | ISO, copyrighted (sample pages say "COPYRIGHT PROTECTED DOCUMENT") | Restate facts; quote short phrases; free sample pages only |
| BS 1710 | BSI, copyrighted; the Water Regs UK extract says "No other use of this material is permitted" | Restate facts; link the leaflet |
| DIN 2403 | DIN, copyrighted | Restate facts; TRGS 201 Annex 3 is the public restatement |
| TRGS 201 | Published by the Federal Ministry of Labour in the GMBl | Readable free; restate and quote |
| MIL-STD-101C, MIL-STD-1247D | US Department of Defense, "Approved for public release; distribution is unlimited" | US government work; quote and restate freely, cite |
| NASA-STD-3001, NASA-STD-5005, GSFC-STD-8006, SSP 50005 | NASA, "APPROVED FOR PUBLIC RELEASE" | Same as above |
| Vendor pages | Their authors | Secondary only; cite, do not copy |

Colour assignments ("water is green") are facts and can live in our own data
files, written in our own words with a citation. Tables, figures and the
chromaticity coordinate tables should not be copied wholesale; we pick our
own render colours inside each named colour, which ISO 14726 explicitly
tolerates ("colours of a similar shade and tone may also be used", F10).

---

## I. The same colour, seven meanings (computed from F4 to F27)

| Colour | ISO 14726 (ships) | ISO 20560-1 | ASME A13.1 (per GSFC-STD-8006) | BS 1710 (F17; F18 secondary) | DIN 2403 (per TRGS 201) | MIL-STD-1247D (aerospace lines) | MIL-STD-101C (DoD ground) |
|---|---|---|---|---|---|---|---|
| Green | sea / outside water | water | water | water | water | breathing oxygen | oxidizing |
| Blue | fresh water | air | compressed air | light blue: air; auxiliary blue: public-supply water | oxygen | coolant | anesthetic, harmful |
| Red | fire fighting | firefighting | fire quenching | fire (safety colour) | **steam** | **fuel** | fire protection |
| Yellow | flammable gases | hazardous (safety colour) | flammable, oxidizing | warning (safety colour); ochre: gases | gases | lubrication | flammable |
| Orange | oils other than fuel | acids | toxic, corrosive | electrical | acids | compressed gas | (cylinders only) |
| Brown | fuel | (not used) | combustible | oils | liquids, solids | fire protection | toxic, poisonous |
| Grey | non-flammable gases | gases | GSFC: physically dangerous | silver-grey: steam | air | de-icing | physically dangerous |
| Violet / purple | acids, alkalis | alkalis | GSFC: anesthetic, harmful | acids, alkalis | alkalis | (not used) | (not used) |
| Black | waste media | liquids, solids | GSFC: no meaning | other liquids | additional colour: non-flammable | (lettering) | no meaning |
| White | ventilation air | (label background) | GSFC: no meaning | (not read) | (lettering) | (label background) | drinking water |

The A13.1 column's grey, purple, black and white rows are GSFC's assignments;
A13.1 itself leaves four colour slots "Defined by the User" (F5), whose colours
we did not read from a primary source.

Only red-for-fire and green-for-water come close to universal, and even those
break. A colour without a word is not a safe message in any of these schemes.

---

## What is still unknown, and what would settle it

1. **ASME A13.1's current colour table.** The 2023 edition revised "Table
   4.2-1, Designation of Colors" (F3) and a 2026 edition is listed (F1). We
   read neither. Settle by buying ASME A13.1-2026 (USD 50, PDF, F1).
2. **ISO 14726 clause 5 "Design" and Annex B.** Band widths, the order on the
   pipe, legends, arrows, and the RAL or other equivalents of each colour are
   not in the free pages. Settle by buying ISO 14726:2008 (13 pages) or reading
   it at a library holding ISO standards.
3. **ISO 20560-1:2024's changes and its Annex E "Maritime piping systems".**
   In particular, how it reconciles its blue (air) and green (water) with ISO
   14726's blue (fresh water) and green (sea water). Settle by buying ISO
   20560-1:2024 (25 pages).
4. **BS 1710's full basic colour table.** Two vendor pages disagree on steam,
   other liquids and electrical (F18). Settle by reading BS 1710:2014.
5. **Whether DIN 2403:2025-12 changed the colour groups** that TRGS 201 gives
   from the 2014 edition. Settle by reading DIN 2403:2025-12 (EUR 90.50 per
   DIN Media) or a later TRGS 201 amendment.
6. **What the ISS actually uses on its fluid lines.** SSP 50005E requires
   labels and keyed connectors (F25) but no line colours were found. Settle
   with the ISS labelling documents (the decal catalogue and labelling plan
   that NASA-STD-3001 refers to as a "Labeling Plan and Icon Library") or
   with documented ISS interior photographs. Also unknown: which standard, if
   any, replaced MIL-STD-1247 for new aerospace designs (it is "inactive for
   new design", F22).
7. **Whether any flag state or classification society makes ISO 14726
   mandatory on ships.** Not researched. Settle with the rules of one or two
   classification societies.
8. **Whether plastic pipe colours (PEX red and blue, PVC white or grey, CPVC,
   HDPE black) are fixed by product standards** (ASTM F876, D1785 and others)
   or are only conventions. Settle by reading those standards' marking
   clauses.
9. **ISO 14726 additional-colour codes in F12** came from a text extraction
   (checked two ways, see "How the sources were handled"). Confirm against the
   printed standard before they become game data.

## Sources that could not be reached

- ISO Online Browsing Platform previews
  ([iso.org/obp](https://www.iso.org/obp/ui/#iso:std:iso:14726:ed-1:v1:en)):
  HTTP 403 to our fetch tool. Wanted: ISO 14726 clause 5 and ISO 20560-1:2024.
- ANSI webstore preview of ISO 20560-1:2024
  ([webstore.ansi.org](https://webstore.ansi.org/preview-pages/ISO/preview_ISO+20560-1-2024.pdf)):
  returned a bot-check page; not bypassed. Wanted: the 2024 changes.
- Standards Norway viewer for ISO 20560-1:2020 (forhandsvis.standard.no):
  JavaScript only, no text. Wanted: Annex E.
- Brady EU ISO 20560 page
  ([brady.eu](https://www.brady.eu/pipe-markers-and-valve-tags/iso-20560)):
  HTTP 403. Wanted: a vendor cross-check of the ISO 20560-1 groups.
- GlobalSpec pages for A13.1
  ([standards.globalspec.com](https://standards.globalspec.com/std/14639424/a13-1)):
  HTTP 403. Wanted: the 2026 edition's publication date.
- Accuris previews of A13.1 (store.accuristech.com): HTTP 403. Wanted: the
  colour table.
- Copper Development Association live pages (copper.org tube tables):
  HTTP 404 and 403; the Internet Archive capture was used instead (F31).
- Bureau of Indian Standards draft PDFs (services.bis.gov.in): files gone.
- NASA-STD-3001 Vol 2 Rev D on nasa.gov: empty response; Rev F was read
  instead (F26).
- KSC-STD-SF-0004B PDF: downloaded, but a scanned image with no text layer.
- MIL-STD-1247D at live.expresscorp.com: "Error code: 522"; another copy was
  used (F22).
- NASA MSIS (msis.jsc.nasa.gov, the old NASA-STD-3000): no response.

---

## What it means for this project

### Certain (from the sources and from our own code)

- **Real international schemes exist for content marking.** ISO 14726 (ships,
  confirmed 2024) and ISO 20560-1 (general, 2024, "can also be used for
  marine structures and ships"). We do not need to invent a scheme.
- **No scheme marks the wall material.** Copper, steel and plastic show
  themselves; makers print their own grade markings (F31).
- **Colour must be backed by words** in every scheme, and by an "additional
  cue" in NASA's current spacecraft standard (F26).
- **Markers may be bands or labels at intervals**, with the real or painted
  pipe between them (F5, F8, F17, F29).
- **Our code cites a standard it does not follow.**
  [`data/routing_rules.ron`](../../../data/routing_rules.ron) line 5 and
  [`src/systems/construction/routing.rs`](../../../src/systems/construction/routing.rs)
  line 12 say the routing is "Grounded in ... ASME A13.1 (pipe color)". The
  colours actually come from `MachineHome::connection_color`
  ([`src/machines.rs`](../../../src/machines.rs) line 2338), where water is
  blue (A13.1 says green), hot water is red (red is fire fighting in ISO
  14726, ISO 20560-1, A13.1, BS 1710 and MIL-STD-101C), power is amber yellow
  (yellow is flammable or a hazard in those same five; electrical conduits
  are outside A13.1 and ISO 14726, and BS 1710 makes them orange), data is
  violet (acids and alkalis in ISO 14726, ISO 20560-1, BS 1710 and DIN 2403),
  nutrient is brown (fuel in ISO 14726), and waste is grey-green (black in ISO
  14726).
- **Our pipes hide their material.** [`src/engine/home_meshes.rs`](../../../src/engine/home_meshes.rs)
  lines 1973 to 1984 paint the whole pipe in that utility colour with a
  little emissive glow, varying only metal and roughness between rigid and
  flexible kinds.

### Judgement calls (ours; a reader may weigh them differently)

**Which scheme for what.**

- **Content colour aboard the ship: ISO 14726.** It is the standard for the
  setting the game is in, a ship. It has the vocabulary a ship's systems need
  (fresh water, outside water, fuel, oils, steam, ventilation air, sounding
  pipes, waste), and its main-plus-additional triples give spare codes for
  the game's own media. "Sea water" has no sea in space; ISO 14726's own
  footnote widens green to "all outside waters", so green can mean water
  taken from outside the hull (mined ice, a comet) before treatment. A player
  who learns the ship's code aboard is learning the real marine code.
- **The label: ISO 20560-1's non-colour elements.** Content name (sans serif,
  mixed case, contrast colour or black on white), a single or double-headed
  flow arrow, and GHS hazard pictograms for hazardous contents, without the
  gas-cylinder pictogram (TRGS 201 and GSFC both say to drop it). Do NOT use
  ISO 20560-1's colours aboard, because its blue and green mean air and water,
  the opposite of ISO 14726's on those two.
- **Placement: the rule every scheme shares.** At every valve and operating
  accessory, at branches, changes of direction and wall or deck
  penetrations, and at intervals so that at least one marker is visible from
  anywhere along the run (MIL-STD-1247D's wording, F22). GSFC's 20 ft
  (about 6 m) interval indoors is a good default for data.
- **Electrical conduit: orange band plus voltage.** Neither A13.1 nor ISO
  14726 covers conduit; BS 1710 uses orange (F18) and MIL-STD-1247D wants the
  usage and voltage written on the conduit (its 5.6.1). Amber yellow should
  go, because yellow means flammable.
- **Spacecraft rules on top:** connectors of a different shape or key per
  content, so a fuel line cannot be plugged into a water inlet (F24, F25,
  F27), and both halves of a connection carry a matching code.
- **The Real side of the Real/Sim toggle** should offer the user's local
  scheme instead (ASME A13.1 in the US, BS 1710 in the UK, DIN 2403 in
  Germany, ISO 20560-1 elsewhere), because that is the code they will meet in
  their own building. So the scheme should be data, a registry of schemes
  each mapping content to bands and label colours, in the spirit of the
  existing `data/utilities/conduits.ron` registry, with ISO 14726 as the
  ship's default. That also keeps every scheme's data in one place where its
  source and edition can be cited.
- **Two modes, per the project's dual-mode rule:** the simplified mode shows
  one main-colour band with the content name; full realism shows the
  main-additional-main triple, flow arrow, hazard pictograms and pressure or
  temperature where the schemes ask for them. Both modes use the same colours,
  so nothing learned in one is wrong in the other.

**How real material stays visible.**

- A pipe has three layers of data: its **material** (copper, stainless steel,
  carbon steel, galvanised steel, PVC, CPVC, PEX, HDPE, aluminium, titanium
  and so on, with the maker's print line or type stripe as a thin marking:
  "TYPE L" on copper with its blue print, F31); an optional **finish**
  (any paint colour, an insulation jacket, or none); and its **markers**
  (bands and labels from the scheme, placed by the rule above).
- The pipe body renders its material or finish everywhere except under the
  markers. The content colour lives only in the markers, as the schemes allow
  (F5, F8, F17). Painting the whole run in the content colour, as the game
  does now, should become an optional finish, not the default.
- Paint follows the real rules where they exist: a bare carbon steel pipe
  needs a protective finish (MIL-STD-1247D's wording, F22); a finish should
  contrast with the marker colour (BS 1710, F17); a purple pipe means
  non-drinking water in US plumbing (F30), so the Real side should not offer
  purple as a neutral paint there.
- Insulation hides the material, so markers go on the jacket, as real ones
  do.

**What multiplayer could enforce (examples, so the question is concrete).**

- **Honest by construction:** markers are generated from what the simulation
  actually carries, so nobody can label a fuel line "potable water". A wrong
  label is a real-world way to hurt people, and a griefing tool in a shared
  ship. When a line is repurposed, its old markers show as out of date until
  replaced.
- **One scheme per shared space:** the mothership's common systems use one
  server-wide scheme, so every player reads the same code; inside a player's
  own home the owner may choose another scheme, or leave markers off, at
  their own risk.
- **Free choice of material and paint** within what the material allows,
  since none of the standards constrain it.
- **Keyed connectors enforced everywhere:** a fitting for one content cannot
  physically join a line of another. This is cheap to check and is what
  spacecraft do.

Which of these to adopt is the operator's call; the honest-by-construction
markers and keyed connectors are the ones the sources support most directly.

**Again: this is a reading of public sources by people who are not piping
engineers, safety officers or lawyers, dated 2026-10-04.** Standards are
revised (A13.1 has had editions in 2015, 2020, 2023 and 2026). If a source
changes, write a new dated finding that links back to this one.
