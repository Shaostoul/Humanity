# Helper notes H: The C-130 class, heavy lift for 100-tonne vehicles, moorings (README section 2)

Part of the [mothership sizes research](README.md) (2026-11-13). A research helper wrote these
notes on 2026-11-12 from the sources it could read; the README carries the answers and the
judgements. After the helper finished, every cited host's robots.txt was checked: findings
read from a page that refuses Claude's readers are marked REMOVED and their numbers are not used.
Quotations were spot-checked against their sources where the README says so; the rest are the
helper's readings, with the helper's own confidence. The helper's own robots notes and summary
table were written before that check: where they name a REMOVED finding or say a host allowed
reading, the check below the README's "How this was read" section is the one that holds.


Constraint on this run: the shared web-search budget was used up before my first search, so I found pages only by following links from manufacturer pages and by guessing paths. Several makers' own spec pages (Komatsu, Caterpillar) could not be reached; that is recorded as a gap, not filled from memory. Anything marked ASSUMPTION is a design number, not a sourced fact.

## Robots checked
| Host | Result |
|---|---|
| www.af.mil | robots.txt returned 403: could not read, so no af.mil pages used |
| www.nationalmuseum.af.mil | robots.txt returned 403: not used |
| www.lockheedmartin.com | one group `*`; no rule for ClaudeBot, Claude-User, anthropic-ai; product pages and /content/dam PDFs are not disallowed: read |
| www.komatsu.com | `*` disallows only /search/ and /errors/: read (but no 980E-5 page could be found) |
| www.antonov.com | `*` disallows /transport-plane/ and others: NOT read |
| www.volga-dnepr.com | `*` allows /; /files/booklet/ not disallowed: read the An-124 booklet |
| www.cat.com | robots.txt request timed out three times: not read, no cat.com page used |
| www.liebherr.com | `*` disallows only parts shops and admin paths: read |
| belaz.by | no Disallow lines: read |
| www.nasa.gov | `Allow: /`: read (facts page) |
| ntrs.nasa.gov | `Allow: /`: read (API search and PDFs) |
| www.army.mil | `*` disallows /usapa/ etc. only; the one article tried had no Abrams data |
| www.gdls.com | no restrictions; page had no mass or size |
| www.internationaldockingstandard.com | robots.txt returned 403: not used (the IDD came from NTRS instead) |
| battletech.com | `*` disallows /wp-admin/ only: read, nothing on the Atlas or Dire Wolf |
| www.catalystgamelabs.com | robots.txt came back empty; the home page read had nothing on mech size |
| masterunitlist.battletech.com | robots.txt DISALLOWS ClaudeBot and anthropic-ai (whole site): NOT read |
| www.nps.gov, www.boeing.com | robots fetched, nothing then needed |

## Findings

### H1. C-130J-30 Super Hercules size and payload (Lockheed Martin)
- The number(s): C-130J-30 length 34.37 m (112 ft 9 in); height 11.84 m (38 ft 10 in); wingspan 40.41 m (132 ft 7 in). Shorter C-130J (KC/HC/MC) length 29.79 m (97 ft 9 in). Payload (the sheet's table is garbled in extraction; the row order reads) 46,700 lb / 21,183 kg for the -30; takeoff weight 164,000 lb / 74,389 kg.
- Source: C-130J Super Hercules Fast Facts, Lockheed Martin, https://www.lockheedmartin.com/content/dam/lockheed-martin/aero/documents/C-130FastFacts/C-130J_Fast_Facts_.pdf, source's own date: "November 2026"
- Quotation: "Length 112 ft 9 in/34.37 m" and "Wingspan 132 ft 7 in/40.41 m"
- Read: 2026-11-12
- Confidence: high for the three outside sizes; medium for the payload (table columns misaligned in the text extraction, I matched labels to values by order)
- Use for the ship: the C-130-class "tactical" hauler is about 34 m long, 40 m span: a normal bay sized for it is a medium bay.

### H2. C-130J cargo compartment (partial)
- The number(s): the LM-100J brochure shows, in an unlabelled drawing, 10 ft, 9 ft, 55 ft and 10 ft 8 in. These match a hold about 3.0 m wide x 2.7 m high x 16.8 m long, but the drawing's labels were not readable, so I do NOT claim them as sourced. LM-100J (the C-130J-30 freighter) payload (2.5 g) 45,800 lb / 20,775 kg, empty 80,350 lb / 36,446 kg.
- Source: LM-100J Commercial Freighter brochure, Lockheed Martin, https://www.lockheedmartin.com/content/dam/lockheed-martin/aero/documents/LM-100J/LM-100JBrochure_F2018_Website.pdf, source's own date: file name says 2018
- Quotation: "Payload (2.5 g)*....................................... 45,800 lb/20,775 kg"
- Read: 2026-11-12
- Confidence: medium for payload; low for the hold numbers (unlabelled)
- Use for the ship: the C-130 carries about 20 t, nowhere near 100 t. Troop or paratrooper counts were NOT found in the Lockheed pages read (the USAF fact sheet is behind a 403).

### H3. Antonov An-124-100 hold and payload (operator Volga-Dnepr)
- The number(s): maximum payload 120 t (264,600 lb); cargo compartment 6.4 m wide x 4.4 m high x 36.5 m long; length including ramps 43.7 m; volume 1,091 m3; nose door 6.4 x 4.4 m, rear door 6.4 x 3.8 m; empty 180 t; takeoff 392 t.
- Source: An-124-100 specification booklet, Volga-Dnepr Airlines, https://www.volga-dnepr.com/files/booklet/an-124e_final.pdf, source's own date: undated
- Quotation: "Maximum payload, t (lb) 120 (264,600)" and "Cargo Compartment Dimensions, m (inch) 6.4 4.4 36.5"
- Read: 2026-11-12
- Confidence: high (operator's own booklet; the order width x height x length is my reading of the figure beside it, and fits the door sizes)
- Use for the ship: a real aircraft already carries a single 120 t item through a 6.4 x 4.4 m door. Largest single item it takes is a function of hold size, not published here.

### H4. C-5M Galaxy (only partly found)
- The number(s): Lockheed says a payload of "six Mine Resistant Ambush Protected vehicles (MRAPs) or up to five helicopters" and "payload increases of 20 percent over legacy C-5s". No hold dimensions, floor length or single-item limit were found.
- Source: C-5 page, Lockheed Martin, https://www.lockheedmartin.com/en-us/products/c-5.html, undated (the "by the Numbers" graphic is an image and was not readable)
- Quotation: "payload increases of 20 percent over legacy C-5s"
- Read: 2026-11-12
- Confidence: low for sizes (none given); the USAF fact sheet (af.mil) returned 403 on robots so was not read
- Use for the ship: use the An-124 (H3) as the sourced scale; do not quote C-5 hold figures from memory.

### H5. Largest mining trucks: BelAZ 7530 and 7531 series (maker)
- The number(s): BelAZ 75306 (220 t payload): length 13.39 m, width 7.82 m, height 6.65 m, empty 156.1 t, gross 376.1 t. BelAZ 75310 to 7531G (240 t payload): width 8.45 m, height 6.72 m, length 13.39 m (75310) up to 14.80 m (75319, 7531B to 7531H); empty 161.5 t to 174.8 t; gross 401.5 t to 414.8 t. These are the basic configuration. The 75710 (the 450 t model) is not on these pages.
- Source: BelAZ-7530 series and BelAZ-7531 series pages, BELAZ, https://belaz.by/en/products/products-belaz/dumpers/dump-trucks-with-electromechanical-transmission/dumpers-series-7530/ and .../belaz-7531-series/, undated
- Quotation: "Length - 13 390 mm Width - 7 820 mm Height - 6 650 mm" (7530 page; the 7531 numbers are from a table the fetch tool summarised, so paraphrase, no exact quotation)
- Read: 2026-11-12
- Confidence: medium (maker pages; 7531 table paraphrased)
- Use for the ship: a 240 t-class truck is about 14.8 x 8.45 x 6.72 m and 175 t empty.

### H6. Liebherr T 284 (maker)
- The number(s): payload class 363 t; empty vehicle weight 242 t; gross vehicle weight 605 t. Length, width and height are NOT on the page (a brochure PDF is linked).
- Source: T 284 page, Liebherr, https://www.liebherr.com/en-us/equipment/mining-trucks/t284-8361205, undated
- Quotation: "Gross vehicle weight (GVW) - 605.00 t" (from the listing page; the technical-data page gave "Empty vehicle weight (EVW)" 242.00 t as paraphrase)
- Read: 2026-11-12
- Confidence: medium-high on mass; no size
- Use for the ship: the heaviest empty truck found is 242 t. A hauler that carries it EMPTY needs about 250 t of payload; the 100 t figure in the brief is a floor for one vehicle, not a ceiling.

### H7. Komatsu 980E-5 / 930E-5, Caterpillar 797F: NOT FOUND
- No spec page could be reached on komatsu.com (several guessed paths 404, the product tree is script-built) and cat.com timed out every try. No figure recorded. Known from memory only (NOT sourced): these trucks are roughly 15 to 16 m long, 9 to 10 m wide, 7 m high with the body down, and 17 to 18 m high with the body raised. Treat as an assumption for the envelope below.

### H8. BattleTech Atlas and Dire Wolf
- No official size source could be read. The official unit database (masterunitlist.battletech.com) blocks ClaudeBot and anthropic-ai in its robots.txt, so I did not read it. battletech.com and catalystgamelabs.com home and search pages carry no mass or height. The 100-ton class is the operator's own statement in the brief; I could not confirm it from an official page, and no height was found anywhere. Fan wikis not used.
- Use for the ship: size a "100 t walker" class by mass and an assumed envelope (below), not by the named mechs.

### H9. M1 Abrams mass (what a C-5 carries): NOT FOUND
- The two army.mil and gdls.com pages tried had no mass or size. No figure recorded.

### H10. International Docking System Standard: soft-capture ring (NASA)
- The number(s): the soft-capture mating plane is where the guide petals' cone is 1,200 mm (1.2 m) across. The transfer-passage diameter is given only in Figure 3.1.1.1-2 (a drawing), which the text extraction could not read; I do NOT quote it. Max separation loads are in the 1.8 to 16.7 kN range (small spacecraft loads, not heavy cargo).
- Source: International Docking System Standard (IDSS) Interface Definition Document (IDD) Revision F, NASA, https://ntrs.nasa.gov/api/citations/20220011643/downloads/IDSS%20IDD%20Rev%20F%20Final_07262022.pdf, source's own date: "July 2022"
- Quotation: "intersects the conic outline of the Guide Petals at a diameter of 1200 mm."
- Read: 2026-11-12
- Confidence: high for 1,200 mm; passage size unread
- Use for the ship: a standard docking port is a person-sized tunnel under about 1 m, with no heavy-cargo path. Heavy vehicles must not go through a docking port.

### H11. Common Berthing Mechanism ring (NASA)
- The number(s): berthing ring outside diameter about 2.0 m (80 in), inside diameter about 1.8 m (71 in), depth 0.19 m (7.5 in); the ring was widened from 1.68 m (66 in) so utilities could run inside it. The hatch size inside (commonly said to be about 1.27 m / 50 in square) was NOT found in this source.
- Source: Space Station Freedom Common Berthing Mechanism, NASA Technical Reports Server 19920015843, https://ntrs.nasa.gov/api/citations/19920015843/downloads/19920015843.pdf, source's own date: 1992 (the report series)
- Quotation: "diameter of approximately 2.0 meters (80 inches) and inside diameter of approximately 1.8 meters (71 inches)"
- Read: 2026-11-12
- Confidence: high for ring size; hatch unconfirmed
- Use for the ship: the largest real moveable-cargo berth passage is under 2 m wide: it moves racks, not vehicles. A freighter berth for the game needs its own big hatch ("lock door").

### H12. ISS size and docking ports
- The number(s): ISS 109 m end to end, 419,725 kg; "Eight spaceships can be connected to the space station at once."
- Source: Space Station Facts and Figures, NASA, https://www.nasa.gov/international-space-station/space-station-facts-and-figures/, undated
- Quotation: "Eight spaceships can be connected to the space station at once."
- Read: 2026-11-12
- Confidence: high
- Use for the ship: a hull with many ports moors several ships at once; real practice is a fixed count of ports plus free-flyers berthed by arm.

### H13. Airship mooring mast: principle (NACA, 1929)
- The number(s): none numeric. The three requirements for a mooring mast: enough height that the airship touches nothing on the ground, a short rigid bow attachment, and the airship free to turn in "horizontal and rolling movements".
- Source: NACA Technical Memorandum 513, "Contribution to the Technique of Landing Large Airships, Part II", O. Krell, NASA NTRS 19930090872, https://ntrs.nasa.gov/api/citations/19930090872/downloads/19930090872.pdf, source's own date: May 1929
- Quotation: "Entire freedom of the airship in its horizontal and rolling movements"
- Read: 2026-11-12
- Confidence: high that this is the principle; no mast size read
- Use for the ship: a long or odd craft can moor by the nose at a free-swinging post outside the hull (a "mast mooring"), and the ship must have clear swing room all round.

### H14. Ship berth spacing / dolphins: NOT FOUND
- No port design standard (PIANC or similar) was reached; no figure recorded.

## Worked numbers: a heavy-lift hauler for ONE oversize vehicle

Design vehicle (the "haul-truck envelope"). Sourced: BelAZ 7531 up to 14.8 m long, 8.45 m wide, 6.72 m high, 174.8 t empty; Liebherr T 284 242 t empty. The Komatsu and Cat trucks could not be sourced, so ASSUMPTION: one truck envelope of 16.0 m long x 10.0 m wide x 7.5 m high (body down), mass up to 250 t empty. This covers a 100 t walker too.

Loading clearance guide. Sourced An-124: 6.4 x 4.4 m door and hold, 36.5 m long, carries 120 t. Its clearance margin over specific loads was not published, so ASSUMPTION: 0.6 m each side, 0.5 m overhead, 1.0 m at each end for lashing.

Hold (inside):
- Length 16.0 + 1.0 + 1.0 = 18.0 m; round up to 20.0 m for chocks and a loadmaster walkway.
- Width 10.0 + 0.6 + 0.6 = 11.2 m.
- Height 7.5 + 0.5 = 8.0 m.
- Floor 20.0 x 11.2 = 224 m2; volume 1,792 m3. (The An-124 hold is 6.4 x 4.4 x 36.5 m, floor about 234 m2, volume 1,091 m3: the hauler's hold has a similar floor but is almost twice as wide and high.)
- Loading door: 11.2 x 8.0 m (full cross-section) as a rear ramp. Floor rating at least 250 t over a 16 m footprint (about 15.6 t per metre).

Hauler ship (ASSUMPTION): 60 m long (20 m hold, 10 m ramp, 30 m engines and flight deck), 20 m wide (hold plus 4.4 m of structure each side), 14 m high. Footprint 60 x 20 = 1,200 m2.

Bay for it with ICAO code F 7.5 m clearance all round plus 10 m apron (the README's assumption; the F figure belongs to the other helper's finding):
- Width 20 + 7.5 + 7.5 = 35.0 m.
- Length 60 + 7.5 + 7.5 = 75.0 m, plus 10 m unloading apron at the ramp end = 85.0 m.
- Bay floor 35.0 x 85.0 = 2,975 m2 (about 0.30 hectare).
- Bay height 14 + 7.5 = 21.5 m.

For comparison, a C-130J-30-sized craft (34.37 m x 40.41 m span, H1): clear box 49.4 x 55.4 m = 2,736 m2, or 3,290 m2 with a 10 m apron. So the heavy hauler's bay is about the same floor as a tactical-transport bay, because it is long and narrow rather than wide. Against the drum: 20 m high bay in a ship 250 m in radius is a small fraction, but the 85 m bay length is 4 percent of the 2 km length.

Odd-ship mooring (design from H10, H11, H13): do NOT size the bay for oddities. Use outside "mast moorings": a free-swinging nose post on the hull skin, with 1.2 m soft-capture docking rings for small craft and a person-sized 1.8 m berthing ring for crew; cargo uses a separate wide "lock door" of 11.2 x 8.0 m class. Swing room needed around a moored craft is its length plus margin in a circle (an 80 m craft needs a clear circle of about 90 m radius).

## Could not read
- https://www.af.mil/robots.txt and https://www.nationalmuseum.af.mil/robots.txt: 403
- https://www.internationaldockingstandard.com/robots.txt: 403
- https://www.cat.com/robots.txt and Cat 797F page: timeout
- https://masterunitlist.battletech.com: robots disallows ClaudeBot and anthropic-ai
- Antonov's own pages (/transport-plane/ is disallowed); web.archive.org copies not used
- Komatsu 980E-5 and 930E-5 pages: not found on komatsu.com
- Lockheed C-5M spec page: 404 on the guessed URL; only the C-5 overview was read

## Negative findings and gaps
- Web-search budget exhausted before any search; every finding came from links on makers' pages.
- No sourced C-5M hold size or Abrams mass; no C-130J hold size or troop count that is cleanly labelled.
- No sourced Komatsu, Cat or Liebherr truck length, width or height; BelAZ is the only truck with sizes.
- No official BattleTech size or height; the 100-ton class is the operator's word only.
- IDSS transfer-passage diameter and CBM hatch size are in drawings or other documents not read.
- No ship-berth spacing standard and no airship mast dimensions found.

## Summary table
| Question | Recommended number | Findings | Confidence |
|---|---|---|---|
| C-130J-30 size | 34.37 x 40.41 x 11.84 m; about 21 t payload | H1, H2 | high size, medium payload |
| An-124 hold | 6.4 x 4.4 x 36.5 m, 120 t | H3 | high |
| C-5M | no hold data found | H4 | low |
| Largest truck envelope (sourced) | 14.8 x 8.45 x 6.72 m, 175 t empty (BelAZ); 242 t empty (Liebherr) | H5, H6 | medium |
| Truck design envelope (assumed) | 16 x 10 x 7.5 m, 250 t | H5 to H7 | low (assumption) |
| Hauler hold | 20 x 11.2 x 8.0 m, 224 m2 | worked numbers | design |
| Hauler bay | 35 x 85 m = 2,975 m2, 21.5 m high | worked numbers | design |
| Docking port soft-capture | 1.2 m; berthing ring 1.8 m clear | H10, H11 | high |
| BattleTech 100 t walker | no official size read | H8 | gap |
