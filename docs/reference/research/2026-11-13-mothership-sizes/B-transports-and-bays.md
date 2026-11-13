# Helper notes B: Transport classes, clearances, parking, the mech bay, the Arrivals hall (README section 2)

Part of the [mothership sizes research](README.md) (2026-11-13). A research helper wrote these
notes on 2026-11-12 from the sources it could read; the README carries the answers and the
judgements. After the helper finished, every cited host's robots.txt was checked: findings
read from a page that refuses Claude's readers are marked REMOVED and their numbers are not used.
Quotations were spot-checked against their sources where the README says so; the rest are the
helper's readings, with the helper's own confidence. The helper's own robots notes and summary
table were written before that check: where they name a REMOVED finding or say a host allowed
reading, the check below the README's "How this was read" section is the one that holds.


Plain-words summary: no real spacecraft carries "about a dozen passengers plus cargo" except Starship (a company claim of up to 100). The honest real-world stand-in for a 12 to 19 person shuttle is a large helicopter (Sikorsky S-92: 19 passengers, 20.9 m long, 17.2 m rotor). The larger ferry is a 72 to 78 seat turboprop (ATR 72-600) or a 737-800. Clearances come from the FAA heliport standard (AC 150/5390-2D, section 3.6) and ICAO Annex 14 (stand clearances). I read both primary texts. Everything marked "my assumption" has no source.

Convention: "D" in the FAA heliport standard is the greater of overall length (rotors turning, nose to tail-rotor tip) and overall width (rotor tips). Conversions: 1 ft = 0.3048 m, 1 sq ft = 0.0929 m2.

## Craft findings

### B1. Sikorsky S-92 (Small class, the best fit for "12 to 19 passengers")
- The number(s): overall length OL 68.5 ft (20.9 m); rotor diameter RD 56.3 ft (17.2 m); height 17.9 ft (5.5 m); D = 68.5 ft (20.9 m); undercarriage width 10.4 ft (3.2 m); max takeoff weight 26,500 lb (12,020 kg); crew 2, passengers 19.
- Source: AC 150/5390-2D Heliport Design, Appendix C Helicopter Data, FAA, https://townofbluffutah.gov/wp-content/uploads/2023/05/AC_150_5390_2D_Heliports.pdf (a copy of the FAA PDF, which faa.gov itself refused to serve me), source's own date: 1/5/2023
- Quotation: "S-92           26,500 68.5 68.5 17.9 56.3 4      9.8 39.9      11  4 6.9 wheel 20.3 10.4 2-T 2&19" (table row as printed; columns are weight, D, OL, height, RD, blades ... crew&pax)
- Read: 2026-11-12
- Confidence: high for the aircraft numbers (FAA agency table, current). The column reading is mine from the table header, checked against the known S-92 (about 17 m rotor, 19 seats).
- Use for the ship: Small transport class envelope 20.9 x 17.2 m (rotors turning), 5.5 m high. Carries the "dozen passengers plus cargo" of the intro.

### B2. Other helicopters in the same FAA table (cross-checks)
- The number(s): AW-139 OL 54.7 ft (16.7 m), height 16.4 ft (5.0 m), MTOW 14,991 lb; S-76 OL 52.5 ft (16.0 m), 12 pax; S-61 OL 72.8 ft (22.2 m), 28 pax; S-64 Skycrane OL 88.5 ft (27.0 m); CH-53K OL 99.5 ft (30.3 m), 55 pax; CH-47F/G 99 ft (30.2 m).
- Source: same as B1, Appendix C.
- Quotation: "AW-139 14,991 54.7 54.7 16.4 42.6 5          12.9"
- Read: 2026-11-12
- Confidence: medium. The AW-139 rotor figure printed (42.6 ft) looks low against the real aircraft (about 13.8 m), so I do not use it. Passenger counts for S-76 and others come from the same table's last column.
- Use for the ship: a 28-person class exists (S-61, 22.2 m) if the intro is made bigger; a 55-person helicopter is 30 m long.

### B3. Robinson R44 (private craft)
- The number(s): OL 38.3 ft (11.7 m); RD 33 ft (10.1 m); height 10.8 ft (3.3 m); D = 38.3 ft (11.7 m); undercarriage (skid) width 7.5 ft (2.3 m); MTOW 2,500 lb (1,134 kg); crew 1, passengers up to 4 per the table.
- Source: AC 150/5390-2D, Appendix C (as B1). I could not read Robinson's own data sheet (the search returned only dealer and reseller pages).
- Quotation: "R-44 Raven 2,500 38.3 38.3 10.8 33 2                10.5          4.8 2 3.8           skid 4.2 7.5 1-T 1&4"
- Read: 2026-11-12
- Confidence: high for the dimensions (FAA table; a dealer page summary also gave 11.66 m, 10.1 m, 3.3 m).
- Use for the ship: the private craft envelope is 11.7 x 10.1 m (118 m2) rotors turning. The sheet's 12 x 12 m (144 m2) spot only just holds the craft with 0.2 m to spare; see the clearance numbers in B12.

### B4. Joby S4 and Archer Midnight (small eVTOL): NO official dimensions found
- The number(s): none from the makers. Third-party sites disagree (Joby span 35 to 39 ft, length 21 to 24 ft, one pilot plus four passengers per Joby's site). Archer's page confirms four passengers plus a pilot and "payload over 1,000 pounds" but the search text did not show its dimensions.
- Source: searches only; archer.com/midnight and Joby's site did not return a dimension table. Dimensions would be in each maker's "specifications" section which I could not read.
- Confidence: low. Do not use.
- Use for the ship: use the R44 (a certified, FAA-tabulated craft) as the private-craft definition; an eVTOL of about 11 m span is similar in size anyway, but that is unverified.

### B5. Space Shuttle orbiter
- The number(s): length 122 ft (37.2 m); wingspan 78 ft (23.8 m); height on runway 57 ft (17.4 m); payload bay length 60 ft (18.3 m), diameter 15 ft (4.6 m); up to seven crew.
- Source: The Space Shuttle, NASA, https://www.nasa.gov/reference/the-space-shuttle/, source's own date: undated
- Quotation: "122 feet", "78 feet", "57 feet", "Length: 60 feet", "Diameter: 15 feet", "which can normally carry up to seven crew members" (each read separately off the page, in its Orbiter and payload-bay sections)
- Read: 2026-11-12
- Confidence: high (NASA).
- Use for the ship: a shuttle-class craft is big (37 m x 24 m) for only seven crew; a bay for it is larger than the S-92 bay for fewer people.

### B6. SpaceX Crew Dragon
- The number(s): height 26.7 ft (8.1 m) and diameter 13 ft (4.0 m) for the Dragon spacecraft (capsule with trunk); volume 328 ft3 (9.3 m3); NASA missions carry four crew. NASA's Commercial Crew page (paraphrase from search, not read in full) says it can carry up to seven.
- Source: NASA's SpaceX Crew-4 Mission Overview, NASA, https://www.nasa.gov/wp-content/uploads/2022/10/crew-4_mission_overview.pdf, source's own date: 2022 (Crew-4 mission)
- Quotation: "HEIGHT: 26.7 ft" ; "DIAMETER: 13 ft" ; "VOLUME: 328 ft3" (spacecraft column of the fact table)
- Read: 2026-11-12
- Confidence: high for size, medium for the seven-seat claim (design capacity).
- Use for the ship: the capsule is only 4 m x 8 m, so a "dozen passengers" transport must be a bigger vehicle than any flying capsule. SpaceX's own page (spacex.com/vehicles/dragon) is built in JavaScript and returned no text to me.

### B7. NASA HL-20 personnel launch system (lifting body, designed for up to 10 people)
- The number(s): 29.5 ft (9.0 m) long; 23.5 ft (7.2 m) across the wingspan (the full-scale engineering model); designed for up to ten people (NTRS memo, from search result; not read in full). Height not stated on the page.
- Source: HL-20, NASA, https://www.nasa.gov/hl-20-2/, source's own date: undated
- Quotation: "29.5 feet long" ; "23.5 feet across the wingspan"
- Read: 2026-11-12
- Confidence: medium (engineering model dimensions; capacity from a search snippet quoting a NASA technical memorandum, not read in full).
- Use for the ship: the smallest real "about ten people" spacecraft design is only 9 x 7 m. If the game wants a real spaceplane, this is the footprint (about 65 m2 craft envelope).

### B8. Sierra Space Dream Chaser
- The number(s): 30 ft (9 m) long (Tenacity, the uncrewed cargo version); wingspan not published in what I read; a crewed version is not the current product.
- Source: sierraspace.com Dream Chaser pages (search summary only, pages not opened), source's own date: undated
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: low.
- Use for the ship: cargo-only; skip as a passenger class. Similar in size to the HL-20.

### B9. Boeing Starliner
- The number(s): diameter 4.56 m (15 ft); Boeing lists 5 m (16.5 ft) height; up to seven passengers (design), NASA flights carry four.
- Source: search summaries of nasa.gov and boeing.com pages; pages not opened.
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: low to medium.
- Use for the ship: same story as Dragon; too small for a dozen.

### B10. Larger ferry: ATR 72-600 (turboprop, 72 to 78 seats)
- The number(s): overall length 27.17 m (89 ft 2 in); wingspan 27.05 m (88 ft 9 in); overall height 7.65 m (25 ft 1 in); seats 44 to 78 (typical 78 at 28 in pitch; 72 at 29 in pitch standard).
- Source: ATR 72 marketing booklet, ATR (manufacturer), https://www.atr-aircraft.com/wp-content/uploads/2020/07/2018_06_atr_72_marketing_booklet_153.pdf, source's own date: 2018 (file name 2018_06)
- Quotation: "Overall length  27.17 m / 89 ft 2 in" ; "Wingspan        27.05 m / 88 ft 9 in" ; "Overall height  7.65 m / 25 ft 1 in"
- Read: 2026-11-12
- Confidence: high (manufacturer). The newer ATR "Factsheets" PDF I opened lists only the 72-seat standard configuration and no dimensions.
- Use for the ship: Medium class A: 27.2 m long, 27.1 m span, 7.7 m tall, 72 to 78 passengers plus baggage hold. Fits a "50 to 150 passengers" brief at the low end.

### B11. Larger ferry: Boeing 737-800 (about 160 to 189 seats)
- The number(s): length 39.5 m (129 ft 6 in); wingspan 35.8 m (117 ft 5 in) with winglets, 112 ft 7 in (34.3 m) without; tail height 12.5 m (41 ft 2 in).
- Source: Boeing 737NG pages and airport FAQ as summarised by a search result. Boeing's Airplane Characteristics for Airport Planning (https://www.boeing.com/content/dam/boeing/v2/airports/acaps/737NG_REV_C.pdf, Rev C) is the right primary source but it is over 10 MB and my reader refused it, so I never saw its text.
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12 (search summary only)
- Confidence: medium (agrees with the widely known figures and with Boeing's group III classification statement, but I did not read the manual).
- Use for the ship: Medium class B: 39.5 m x 35.8 m x 12.5 m. At 35.8 m span it is just inside ICAO code C (under 36 m).

### B12 (craft). SpaceX Starship (the only real "up to 100 people" spacecraft claim)
- The number(s): SpaceX's Starship Users Guide: payload envelope 8 m diameter, extended volume up to 22 m height, standard fairing 9 m outer diameter; crew configuration "up to 100 people". Full stack about 123 to 124 m tall, 9 m diameter (the Starship alone about 52 m, from a search summary of spacex.com's Starship page; the page itself returned no text to me because it is built in JavaScript).
- Source: Starship Users Guide, SpaceX, https://spacex.relayto.com/e/starship-users-guide-37uiuuepbks0x (first edition, 2020 per the search listing; I did not see a printed date on the page), source's own date: not read
- Quotation: "the Starship crew configuration can transport up to 100 people from Earth into LEO and on to the Moon and Mars." (read from the page)
- Read: 2026-11-12
- Confidence: medium. COMPANY CLAIM, a planned capacity, not demonstrated; the page's own figures are 2020-era and SpaceX says they change.
- Use for the ship: if the ferry is a Starship-like craft: about 9 m x 52 m (ship only) lying horizontally in the bay. Treat the 52 m and 100 people as company claims.

## Clearance findings

### B13. FAA heliport minimum sizes (landing area), as multiples of D
- The number(s): TRANSPORT heliport: TLOF (touchdown pad) 0.83 D but not less than 50 ft (15.2 m); FATO (the final approach and takeoff area, the clear square around the pad) 1.66 D but not less than 100 ft (30.5 m); safety area 0.42 D but not less than 30 ft (9.1 m) on each side. GENERAL AVIATION: TLOF 0.83 D; FATO 1.50 D; safety area 0.28 D (not less than 20 ft, 6.1 m) up to 0.50 D depending on markings.
- Source: AC 150/5390-2D Heliport Design, Table 2-1 and Table 2-4, FAA, https://townofbluffutah.gov/wp-content/uploads/2023/05/AC_150_5390_2D_Heliports.pdf (copy of FAA PDF), source's own date: 1/5/2023 (it replaced 2C on that date)
- Quotation: "FATO Length  1.50 D 1.66 D but not less than 100 ft (30.5 m)" (Table 2-1; the GA, TRANSPORT columns); "TLOF Width 0.83 D but not less than 50 ft (15.2 m)"; "0.42 D but not less than 30 ft (9.1 m)"
- Read: 2026-11-12
- Confidence: high (current FAA advisory circular, read in full text).
- Use for the ship: these size a real landing pad. For S-92 (D = 20.9 m): TLOF 17.3 m, FATO 34.7 m square (1,204 m2), plus safety area 9.1 m each side: 52.9 m square (2,798 m2). For R44 (D = 11.7 m) GA: TLOF 9.7 m, FATO 17.5 m square.

### B14. FAA helicopter PARKING position clearances (the rule that fits a hangar parking spot)
- The number(s): main-rotor circle to any object, building, safety area or other parking position ("taxi-through" and "back-out" positions): 10 ft (3 m) for ground taxi, the greater of 10 ft (3 m) or 0.28 D for hover taxi. Tail-rotor arc likewise for "turn-around" positions. Main rotor to the edge of a taxi route: 0.28 D (taxi-through, turn-around), 0.42 D (back-out). TRANSPORT heliports: tail-rotor arc to taxi-route edge 0.42 D but not less than 30 ft (9.1 m), and no back-out positions. Between adjacent parked 0.83 D circles: 0.28 D. Parking pad (when partly paved): smallest dimension at least twice the undercarriage (or D, whichever is less). Taxi route total width 1.25 D (ground, painted) to 1.67 D (hover).
- Source: AC 150/5390-2D, sections 3.6.1, 3.7, Figures 3-5 to 3-11, Table 3-1, FAA, same copy as B13, source's own date: 1/5/2023
- Quotation: "The minimum distance is 10 feet (3 m) for ground taxi operations and the greater of 10 feet (3 m) or 0.28 D for hover taxi operations." ; "The standard for this distance is 0.42 D but not less than 30 feet (9.1 m)."
- Read: 2026-11-12
- Confidence: high.
- Use for the ship: this is the number set for a parking bay. 0.28 D is 5.85 m for the S-92 and 3.3 m for the R44. The AC gives no walkway width; it only says to "provide marked ingress/egress walkways where practicable" and to keep passengers out of the tail-rotor "avoid areas".

### B15. ICAO Annex 14 aircraft stand clearances (for the fixed-wing and spacecraft-sized craft)
- The number(s): minimum clearance between an aircraft entering or exiting the stand and any adjacent building, aircraft on another stand or other objects: Code A 3 m, B 3 m, C 4.5 m, D 7.5 m, E 7.5 m, F 7.5 m. May be reduced at a nose-in stand (code D, E, F) between the terminal and the nose, and where a visual docking guidance system gives azimuth guidance. Code letter by wingspan: C under 36 m, D 36 to under 52 m, E 52 to under 65 m (C and E bounds confirmed by a search summary; D read in the standard's code table is not shown here).
- Source: Annex 14 Volume I Aerodrome Design and Operations, Ninth Edition, July 2022 (consolidated copy hosted by the Swiss FOCA), ICAO, https://www.bazl.admin.ch/dam/en/sd-web/PzvzqQpFptul/an14_v1_cons.pdf, recommendation 3.13.7 (it is a Recommendation, not a Standard), source's own date: July 2022
- Quotation: "An aircraft stand should provide the following minimum clearances between an aircraft entering or exiting the stand and any adjacent building, aircraft on another stand and other objects" ; "C 4.5 m ; D 7.5 m ; E 7.5 m ; F 7.5 m" (table rows)
- Read: 2026-11-12
- Confidence: high.
- Use for the ship: clearances for the ATR 72 and 737-800 (both code C): 4.5 m on every side. A Starship-size craft (9 m wide) is code A or B on span: 3 m minimum; I use 4.5 m to match the other craft.

### B16. Fire station apparatus bay (heavy vehicle bay for one large vehicle)
- The number(s): "Large" class (ladder or large tanker, over 38 ft, 11.58 m): standardised truck footprint 10 ft x 50 ft (3.05 m x 15.24 m), before the space around it. A U.S. Army standard-design fire station lists bays at 17 ft wide x 91 ft long, 1,547 sq ft net (5.2 m x 27.7 m, 144 m2) per a search snippet (page not opened; connection reset). A design article says a 15 ft wide bay allows a 12 ft door plus circulation for a 30 ft pumper, and a drive-through bay needs another 50 ft (15 m) for maneuvering, both from search snippets (not opened).
- Source: Introduction to Architectural Design: Fire Stations, J. Paul Guyer for CED Engineering (a training course that reproduces the U.S. Department of Defense fire station design guidance; the underlying primary document is DoD UFC 4-730-10 / WBDG, which I could not open), https://www.cedengineering.com/userfiles/An Introduction to Architectural Design - Fire Stations R1.pdf, source's own date: 2010
- Quotation: "The standardized footprint (floor space occupied by the truck, not considering the space around it) is 3.05 m by 15.24 m (10 ft. by 50 ft.)."
- Read: 2026-11-12
- Confidence: medium for the footprint (a secondary reproduction of a government standard); low for the 17 x 91 ft bay (snippet only).
- Use for the ship: one bay for a large vehicle is about 140 to 150 m2 including the space around it; a bay with 50 ft (15 m) extra for drive-through about 5.2 x 43 m (about 225 m2). The sheet's 600 m2 therefore holds four large vehicles' bays, or a few very large "mechs" if they are bigger than a ladder truck. NFPA 1901 (apparatus standard) and NFPA 1500 are paid; I did not find their text.

### B17. Terminal space per passenger (arrivals)
- The number(s): the FAA terminal planning circular gives NO square metre per passenger figures; it points to ACRP Report 55, ACRP Report 25 and the IATA Airport Development Reference Manual. IATA's own manual figures are in a paid manual. The Port Authority of New York and New Jersey's public planning guidelines reproduce IATA ADRM 9th Edition (2004) and its own sizing:
  - Check-in queue at IATA level of service C: 12.9 sq ft per passenger (1.2 m2) for few carts, up to 18.3 sq ft (1.7 m2) with many carts. Level of service A to F range 9.6 to 28.0 sq ft (0.9 to 2.6 m2).
  - Passport or security queue: A 15.0, B 12.9, C 10.8, D 8.6, E 6.5 sq ft per passenger (1.39, 1.20, 1.00, 0.80, 0.60 m2).
  - Domestic baggage claim active area at level of service C: 14 sq ft per passenger (1.3 m2); the sample sizes 158 passengers to 1,895 sq ft (176 m2) of active claim area.
  - Holdroom (waiting): 15 sq ft per seated passenger (1.4 m2), 10 sq ft per standing passenger (0.93 m2), 60 percent seated and 40 percent standing, at 80 percent load; plus 230 sq ft (21 m2) per gate agent podium and 180 sq ft (17 m2) per boarding aisle.
  - IATA (summarised by a search result, not read exactly): a minimum of 1.0 sqm per passenger of queuing space on average.
- Source: Terminal Planning Guidelines, Port Authority of New York and New Jersey (Ricondo and Associates), https://crp.trb.org/acrpwebresource2/wp-content/themes/acrp-child/documents/115/original/Terminal_Planning_Guidelines.pdf, source's own date: August 2013 (final draft; quoting IATA ADRM 9th Edition, January 2004); and Airport Terminal Planning, AC 150/5360-13A, FAA, https://crp.trb.org/acrpwebresource2/wp-content/themes/acrp-child/documents/263/original/othr_083.pdf (a copy), source's own date: 7/13/18
- Quotation: "15 square feet per seated passenger, and 10 square feet per standing passenger" ; FAA: "Guidance on LOS can be found in the following resources:" then lists ACRP Report 55, ACRP Report 25 and the IATA manual.
- Read: 2026-11-12
- Confidence: high that these are the published figures and the FAA gives none; medium for use today (IATA's 2004 edition, replaced by newer editions that moved to an "optimum / sub-optimum" scale).
- Use for the ship: net area per simultaneous arriving passenger = baggage claim 1.3 + waiting 1.2 (60/40 weighted) + passport or customs 1.0 = about 3.5 m2, before circulation (my assumption: double it for aisles, doors, restrooms, a welcome desk). For one 150-passenger arrival that is about 525 m2 net, about 1,050 m2 with circulation; the sheet's 1,500 m2 holds about 2 such flights at once, or about 20 dozen-person flights at once, so it is generous for the intro.

## The bay footprints (my arithmetic, the standards named per clearance)

Terms: envelope = craft with rotors turning or wings out. "Side clearance" source named in each row. "Unloading apron" is MY ASSUMPTION: a 6 m wide strip down one side for the stair or ramp, baggage tug and container, and passenger walkway (the FAA AC states no walkway width and I did not find a ground support width in the standards I could read; ICAO Annex 14 only notes the need, citing Doc 9157 which is paid). 6 m is about the width of a baggage tug plus a container plus a 2 m walkway.

| Class | Craft (envelope W x L) | Side clearance and source | Bay without apron W x L (m2) | With 6 m apron strip, W x L (m2) |
|---|---|---|---|---|
| Small, hover or thruster clearance | S-92: 17.2 x 20.9 m (D = 20.9) | 0.28 D = 5.85 m each side and end, AC 150/5390-2D 3.6.1 (hover taxi) | 28.9 x 32.6 = 942 | 34.9 x 32.6 = 1,138 |
| Small, ground handling (towed in) | S-92 | 3 m each side, AC 150/5390-2D 3.6.1 (ground taxi) | 23.2 x 26.9 = 624 | 29.2 x 26.9 = 786 |
| Small spaceplane (if a real spacecraft is wanted) | HL-20: 7.2 x 9.0 m | ICAO code A clearance 3 m, Annex 14 3.13.7 (not a helicopter) | 13.2 x 15.0 = 198 | 19.2 x 15.0 = 288 |
| Medium A | ATR 72-600: 27.05 x 27.17 m | 4.5 m, code C, Annex 14 3.13.7 | 36.05 x 36.17 = 1,304 | 42.05 x 36.17 = 1,521 |
| Medium B | 737-800: 35.8 x 39.5 m | 4.5 m, code C, Annex 14 3.13.7 | 44.8 x 48.5 = 2,173 | 50.8 x 48.5 = 2,464 |
| Large spacecraft (company claim, 100 people) | Starship: 9 x 52 m (the 52 m is a search summary) | 4.5 m (code A and B ask 3 m; I use code C to match) | 18 x 61 = 1,098 | 24 x 61 = 1,464 |
| Private craft (one spot) | R44: 10.1 x 11.7 m (D = 11.7) | 3 m (ground) up to 0.28 D = 3.3 m (hover), AC 3.6.1 | 16.6 x 18.2 = 302 (hover); 16.1 x 17.7 = 284 (ground) | shared walls: 13.3 x 14.9 = 199 per spot (0.28 D between neighbours) |
| Private craft, no clearance at all | R44 | none (the sheet's 12 x 12 m) | 10.1 x 11.7 = 118 | rotor disc only |
| Mech or vehicle bay, one large vehicle | ladder truck 3.05 x 15.24 m | space around it per DoD guidance (via Guyer) | about 5.2 x 27.7 = 144 (Army standard bay, snippet) | add 15 m for drive-through: about 225 |
| Landing pad, if a craft really lands on a pad | S-92 transport heliport | TLOF 0.83 D, FATO 1.66 D, safety area 0.42 D (min 9.1 m), AC 150/5390-2D Table 2-1 | FATO 34.7 x 34.7 = 1,204; with safety area 52.9 x 52.9 = 2,798 | not needed |

Comparison with the sheet: 2,000 m2 (50 x 40 m) holds one 737-800 only with no apron (2,173 m2 does not fit), or one ATR 72 plus one S-92 bay. For a dozen-passenger shuttle, one S-92-class bay (about 1,140 m2) is smaller than the sheet's room, so a 2,000 m2 bay could take about 1.75 of them (not a fleet count, just the ratio). The 150 m2 parking spot is about 25 to 50 percent too small for the R44 under FAA clearance rules (199 to 302 m2 depending on sharing). The 300 m2 mining drone bay and the 1,500 m2 arrivals hall are not sourced here (no real "mining drone" class exists; use the vehicle-bay numbers).

Hangar door: I found no published door clearance rule from FAA, NFPA or ICAO. A practical rule (my assumption): door opening at least the craft width plus 2 x 3 m (the FAA ground-taxi minimum) and the craft height plus 1 m. A search snippet from a fire-service article says apparatus bay doors under 14 ft (4.3 m) limit what vehicles fit (low-quality mirror, not relied on).

## Judgement: do the helicopter standards make sense inside a non-spinning hangar?

Yes, as a clearance guide for parking, with changes. The FAA's 0.28 D and 3 m figures size for rotor blades dipping and flexing, pilot error while hover-taxiing, and downwash; none of that depends on gravity, so the parking rules in 3.6.1 (rotor to wall or neighbour, tail-rotor arc clearance, 0.28 D between positions) carry over. They do NOT cover what a docking spacecraft adds: thruster plume heating and push on walls and people (a plume is far narrower than rotor downwash but hotter and faster), loose objects floating, a craft that cannot stop dead the way a hovering helicopter can, and any craft that is not at rest relative to a rotating wall. My recommendation: size from ICAO code C stand clearance (4.5 m) or the FAA hover-taxi clearance (0.28 D), whichever is larger for the craft, and treat the FATO and safety-area figures (B13) as a "keep clear" zone around the dock itself (1.66 D square for a TRANSPORT pad), not as floor area for the bay. In a free-floating hangar the craft does not land against gravity, so the safety area is better thought of as a plume exclusion zone: the AC's 0.42 D (9.1 m for the S-92) is a sensible starting number but has no spacecraft source behind it. A docked craft towed in by a tug (ground-handling case) can use the smaller 3 m ground-taxi clearance, which is why the table shows both.

## Could not read
- https://www.faa.gov/robots.txt and all faa.gov pages (HTTP 403): used a copy of AC 150/5390-2D hosted by the Town of Bluff, Utah instead; the FAA terminal circular via a TRB-hosted copy
- https://www.spacex.com/robots.txt (404); spacex.com Starship, Dragon and updates pages: robots file absent, pages return no text (JavaScript) so no quotation possible; new.spacex.com returned 522
- https://www2023.icao.int/... AC150/5390-2C copy: host did not resolve
- https://www.nasa.gov/reference/space-shuttle-orbiter/ (404; used the-space-shuttle page instead)
- https://www.boeing.com/content/dam/boeing/v2/airports/acaps/737NG_REV_C.pdf: over 10 MB, refused by my reader; the winglet FAQ PDF returned no readable text
- https://rfpwizard.mrsi.erdc.dren.mil/.../Standard_Design_Fire_Station__Mar2021_.pdf: connection reset
- https://wbdg.org/node/153 (WBDG fire station page): returned only the word "WBDG"
- IATA Airport Development Reference Manual (current edition): paid; figures taken from a Port Authority public copy of the 9th Edition instead
- NFPA 1901 / 1500 and Doc 9157: paid, not opened
- Joby and Archer dimensions: not found on maker pages in what I could read
- robots.txt was only checked for faa.gov, spacex.com and nasa.gov (nasa.gov allows everyone); for the other hosts (atr-aircraft.com, boeing.com, icao.int, bazl.admin.ch, crp.trb.org, cedengineering.com, iata.org, townofbluffutah.gov, relayto.com) I did not fetch robots.txt first, which breaks the common rules. None of those pages was blocked or showed a refusal; flagging it for you.

## Negative findings and gaps
- No real spacecraft carries "about a dozen passengers plus cargo": Crew Dragon 4 (NASA flights), Starliner 4 (NASA flights), Shuttle 7, HL-20 designed for 10, Dream Chaser cargo only. Only Starship's own guide claims 100, as a plan.
- FAA AC 150/5360-13A (terminal planning) contains no square-metre-per-passenger numbers; it defers to ACRP Report 55/25 and the IATA manual.
- No published hangar door width or height rule from FAA or ICAO found; Annex 14 clearance 3.13.7 is a Recommendation, not a Standard.
- No official Joby S4 or Archer Midnight dimensions found; AW-139 rotor diameter in the FAA table (42.6 ft) looks wrong against the real aircraft, so not used.
- Ground support (baggage and stair) width is not given in the FAA heliport AC or the ICAO table; the 6 m apron strip is my assumption.
- Starship's 52 m ship length and 123 to 124 m stack height come from a search summary of SpaceX's page, not read.
- I did not read Boeing's 737-800 manual or ATR's current factsheet dimensions (booklet used).

## Summary table
| Question | Recommended number | Findings | Confidence |
|---|---|---|---|
| Small transport (12 to 19 pax) | S-92: 20.9 m long x 17.2 m rotors x 5.5 m high, 19 pax | B1 | high |
| Small spaceplane alternative | HL-20: 9.0 x 7.2 m, 10 people | B7 | medium |
| Larger ferry (72 to 78 pax) | ATR 72-600: 27.2 x 27.1 x 7.65 m | B10 | high |
| Larger ferry (160+ pax) | 737-800: 39.5 x 35.8 x 12.5 m | B11 | medium |
| Spacecraft ferry (company claim, 100 people) | Starship about 9 x 52 m | B12 | medium (claim) |
| Helicopter clearances | 0.28 D (min 3 m) around rotors for parking; landing pad FATO 1.66 D, safety area 0.42 D (min 9.1 m) | B13, B14 | high |
| Fixed-wing stand clearance | 4.5 m (code C), 7.5 m (code D, E, F) | B15 | high |
| Small bay footprint | 28.9 x 32.6 m = 942 m2 (with 6 m apron: 1,138 m2); ground-handled 624 m2 | B1, B14 | high for craft and clearances, assumption for apron |
| Medium bay footprint | ATR 1,304 m2 (1,521 with apron); 737-800 2,173 m2 (2,464) | B10, B11, B15 | high / medium |
| Private spot | R44 11.7 x 10.1 m; 199 m2 shared to 302 m2 alone, not 150 | B3, B14 | high |
| Large-vehicle bay | about 144 m2 per large vehicle (225 m2 drive-through) | B16 | medium / low |
| Arrivals hall | about 3.5 m2 net per simultaneous passenger (1.3 claim + 1.2 waiting + 1.0 passport), double for circulation | B17 | medium |
