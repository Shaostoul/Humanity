# Helper notes F: Drum materials, plates, design rules, and water shielding (README sections 6 and 7)

Part of the [mothership sizes research](README.md) (2026-11-13). A research helper wrote these
notes on 2026-11-12 from the sources it could read; the README carries the answers and the
judgements. After the helper finished, every cited host's robots.txt was checked: findings
read from a page that refuses Claude's readers are marked REMOVED and their numbers are not used.
Quotations were spot-checked against their sources where the README says so; the rest are the
helper's readings, with the helper's own confidence. The helper's own robots notes and summary
table were written before that check: where they name a REMOVED finding or say a host allowed
reading, the check below the README's "How this was read" section is the one that holds.


Research only, read 2026-11-12. Prefix F. Part 1 = structure (F1 to F27), Part 2 = shielding (F28 to F38).
Honesty note: many publisher PDFs would not give text to WebFetch, so I saved the PDF and extracted its text myself; those quotations are exact. Where I only saw a search-result snippet and could not open the page, the finding says so and its confidence is lowered. The search budget ran out near the end, so a few gaps stay open (listed at the bottom).

# PART 1: STRUCTURE AND MATERIALS

## 1A. Strength, stiffness, density

### F1. Structural steel S355 (EN 10025-2), SSAB Multisteel SN
- The number(s): minimum yield 355 MPa up to 16 mm, 345 (16 to 40 mm), 335 (40 to 63), 325 (63 to 80), 315 (80 to 100), 295 (100 to 150). Tensile strength 470 to 630 MPa (450 to 600 for 100 to 150 mm). Elongation 21 to 19 %.
- Source: SSAB Multisteel SN product page, SSAB, https://www.ssab.com/en/brands-and-products/ssab-multisteel/product-offer/ssab-multisteel-sn, undated
- Quotation: paraphrase, no exact quotation (the page is a table; I read the rows through the fetch tool: "5.00 – 16.00 | 355 | 470 – 630 | 21").
- Read: 2026-11-12
- Confidence: medium. A mill's published table of the grade, but I did not read the EN 10025-2 text itself.
- Use for the ship: S355 is the cheap, weldable baseline: 355 MPa thin, falls to 295 MPa at 150 mm. Modulus and density of steel were NOT in any page I read; use 210 GPa and 7,850 kg/m3 as textbook values (unsourced here, see gaps).

### F2. High-strength S690QL (EN 10025-6), ArcelorMittal Industeel declaration of performance
- The number(s): for 100 to 200 mm plate, minimum yield 630 MPa, tensile strength 710 to 900 MPa, elongation 14 % min, impact 30 J at minus 40 C (longitudinal). Thinner plate (search report of the same standard, not opened): 690 MPa up to 50 mm, 650 MPa for 50 to 100 mm; tensile 770 to 940 MPa up to 50 mm.
- Source: Declaration of Performance DOPIL_S690QL_1.8928, ArcelorMittal Industeel, https://industeel.arcelormittal.com/fichier/dopil_s690ql_1-8928-en-2, PDF created 12 November 2025, cites "EN 10025 : 2022"
- Quotation: "Plates S690QL / 1.8928 according to EN 10025-6"
- Read: 2026-11-12
- Confidence: high for 100 to 200 mm (a manufacturer's legal declaration); medium for the thin-plate values (secondary).
- Use for the ship: a 700 MPa class steel keeps 630 to 690 MPa even when thick. It matches the project's Strenx P700 (SSAB's Strenx 700 is stated to conform to S690QL, per a search result I did not open).

### F3. ASTM A514 Grade B quenched and tempered plate, SSAB
- The number(s): yield 100 ksi min = 690 MPa; tensile 110 to 130 ksi = 760 to 895 MPa; available 4.78 to 31.75 mm. Above 2.5 in up to 6 in: 90 ksi yield, 100 to 130 ksi tensile (Chapel Steel summary seen in a search result, not opened).
- Source: SSAB ASTM A514 Grade B page, https://www.ssab.com/en-us/brands-and-products/commercial-steel/structural-steel/astm-a514/grade-b, undated
- Quotation: "has a minimum yield strength of 100 ksi [690 MPa]"
- Read: 2026-11-12
- Confidence: high for yield and tensile; the page gave no density or modulus.
- Use for the ship: same 690 MPa class as S690QL in US units. Thick-section values are lower, same pattern as F2.

### F4. Titanium Ti-6Al-4V (Grade 5)
- REMOVED 2026-11-12: the UPMET datasheet host disallows ClaudeBot (the ATI density, 4.47 g/cm3, is quoted in the README instead). Its numbers are not used anywhere in this research.

### F5. Aluminium 5083-H321 (weldable marine grade)
- The number(s): density 2.66 g/cm3; tensile 317 MPa; yield 228 MPa; modulus 70.3 GPa (tension), 71 GPa (average of tension and compression); elongation 16 %. Specification minimums for H321 plate (a distributor's datasheet citing ASTM B928, seen in a search result, not opened): tensile 305 MPa min; over 40 mm drops to about 285 MPa tensile and 200 MPa yield.
- Source: MatWeb-style datasheet "Aluminum 5083-H116; 5083-H321" as hosted by Aerospace Metals, https://www.aerospacemetals.com/wp-content/uploads/2023/06/Aluminum-5083-H116-5083-H321.pdf, undated
- Quotation: "Data points with the AA note have been provided by the Aluminum Association, Inc. and are NOT FOR DESIGN."
- Read: 2026-11-12
- Confidence: medium. Typical values only; the PDF's table columns were jumbled, so which of 317 and 228 is tensile versus yield rests on 5083's well known values and the unit conversions (46,000 psi and 33,000 psi), not on clean column labels.
- Use for the ship: 5083 is weldable and seawater-proof but weak: about 230 MPa yield, one third of steel, at one third of steel's density, so strength per mass is about equal to S355 and well below S690. Not a drum hull material.

### F6. Aluminium 2219 (aerospace, weldable)
- The number(s): T851 plate 62 ksi tensile = 427 MPa, 46 ksi yield = 317 MPa (QQ-A-250/30, 0.25 to 2.0 in); T8511 extrusion 58 ksi / 42 ksi. Density and modulus not on the page.
- Source: Aluminium Alloy 2219, Aircraft Materials, https://aircraftmaterials.com/data/aluminium/2219.html, undated (robots.txt there returned 404, so no robots rules exist)
- Quotation: "is readily weldable and has good fracture toughness"
- Read: 2026-11-12
- Confidence: medium (supplier reference page quoting specification values).
- Use for the ship: 2219 is the alloy NASA tanks were welded from; about 317 MPa yield. Still half of S690 yield at a third of the density; again no better than steel per mass.

### F7. Aluminium 7075-T6 / T651
- The number(s): T6 sheet 510 to 538 MPa tensile, 434 to 476 MPa yield; T651 plate (6.35 to 101.6 mm) 462 to 538 MPa tensile, 372 to 462 MPa yield. Density 0.101 lb/in3 = 2.80 Mg/m3. Aluminum Association typicals 572 / 503 MPa were reported by a search result but I did not open that page.
- Source: Aluminum 7075-T651/T6 data sheet, Proto Labs, https://www.protolabs.com/media/2tyj14s3/2026-026-cnc-material-data-sheet_aluminum-7075-t651_t6.pdf, 2026; density from aircraftmaterials.com/data/aluminium/7075.html
- Quotation: "T651 Plate Imperial 0.250- 67-78 ... 54-67" (table rows as printed; ksi)
- Read: 2026-11-12
- Confidence: medium. Modulus NOT found on any page I read (gap).
- Use for the ship: the best aluminium for strength (about 400 to 460 MPa yield) but it cannot be welded structurally (a general fact I did not source here) so it only fits bolted or riveted parts, not a pressure drum.

### F8. Basalt fibre, continuous filament
- The number(s): tensile 3,000 MPa (ASTM D2343); modulus 86 to 90 GPa; density 2.63 g/cm3 (DIN 65569); elongation 3.5 %; filament 13 to 19 micron (this product 17 micron). A trade listing (not opened) reports other producers from 2,900 up to 3,800 to 4,800 MPa and 93 to 110 GPa; Wikipedia's summary warns values differ widely between producers.
- Source: Mafic basalt direct roving 17 micron, technical data on Knowde, https://www.knowde.com/stores/mafic/products/mafic-basalt-direct-roving-17-m, undated
- Quotation: "Tensile Strength 3000.0 MPa, Tensile Modulus 86 - 90 GPa, Elongation at Break 3.5%; Density 2.63 g/cm3 (DIN 65569)" (as printed in the listing snippet)
- Read: 2026-11-12
- Confidence: medium. One manufacturer's listing; the fetch tool confirmed the same numbers.
- Use for the ship: use 3,000 MPa, 88 GPa, 2,630 kg/m3 for the dry fibre. That is a bit above S690 in absolute strength terms but about 4 times S690 per kg (3,000 / 2,630 versus 690 / 7,850). The strength of a basalt-fibre COMPOSITE was not found (gap); a comparable carbon case (F9) shows composite strength is about half to 60 % of the fibre's.

### F9. Carbon fibre (and the fibre-to-composite drop)
- REMOVED 2026-11-12: hexcel.com's data sheet path disallows ClaudeBot (the project's Toray T700S figures are used instead). Its numbers are not used anywhere in this research.

## 1B. What it takes to make each

### F10. Steel: the energy now (worldsteel)
- The number(s): energy per tonne of crude steel, 2022: global average 20.99 GJ (5.8 kWh per kg); blast furnace plus oxygen converter (iron ore + coke) 23.98 GJ/t; scrap in an electric arc furnace 10.20 GJ/t; direct-reduced iron in an electric arc furnace 22.37 GJ/t. CO2: 2.33 t per t (blast furnace route), 0.68 (scrap), 1.37 (DRI).
- Source: Sustainability Indicators 2023 report, World Steel Association, https://worldsteel.org/wp-content/uploads/Sustainability-indicators-report-2023.pdf, 2023 report (data 2021 and 2022)
- Quotation: "Energy intensity by production route GJ per tonne of crude steel cast"; row "Scrap-EAF 0.66 0.68 10.07 10.20"
- Read: 2026-11-12
- Confidence: high (industry body's own survey). Includes everything from ore to cast steel, not rolling.
- Use for the ship: plan on about 24 MJ/kg (6.7 kWh/kg) for new steel from ore, about 10 MJ/kg (2.8 kWh/kg) from scrap. Ore + carbon (coke) or hydrogen are the inputs; the ship's drum steel mass (tens of Mt) is the real energy bill, not the process's chemistry.

### F11. Steel with hydrogen reduction
- The number(s): 3.48 MWh electricity per tonne of liquid steel = 12.5 MJ/kg (3.5 kWh/kg), mostly for electrolysing water to make the hydrogen.
- Source: Assessment of hydrogen direct reduction for fossil-free steelmaking, Vogl, Ahman and Nilsson, Journal of Cleaner Production vol. 203, 2018, pp. 736 to 745, https://lup.lub.lu.se/record/47f58242-a20b-4d85-a43d-92c5904590c1, 2018
- Quotation: "hydrogen direct reduction steelmaking needs 3.48 MWh of electricity per tonne of liquid steel"
- Read: 2026-11-12
- Confidence: medium (one modelling study; no independent operating-plant figure found).
- Use for the ship: with clean electricity, steel costs about 3.5 kWh/kg of electricity plus iron ore and scrap-melting overhead. It is the all-electric route suited to a colony.

### F12. Aluminium: the energy
- The number(s): primary aluminium about 70 GJ per tonne (70 MJ/kg = 19.4 kWh/kg, all energy); secondary (recycled) uses 5 % of that.
- Source: Net-Zero Industry Tracker 2023, Aluminium, World Economic Forum with Accenture, https://www3.weforum.org/docs/WEF_Net_Zero_Tracker_2023_ALUMINIUM.pdf, 2023
- Quotation: "The energy intensity of primary aluminium is around 70 GJ/tonne"
- Read: 2026-11-12
- Confidence: medium. A think-tank report citing IAI data, not IAI's own table (IAI's smelting-power page gave definitions but no kWh figure). Electricity-only smelting figure not confirmed (a search result mentioned about 14,000 kWh/t; unread).
- Use for the ship: the process is Hall-Heroult electrolysis of alumina (from bauxite) dissolved in molten cryolite with carbon anodes that burn away (these steps are standard chemistry; I did not source them separately). It needs carbon and fluorine as well as huge electricity: about 3 times steel per kg. Pursue recycling, not new aluminium.

### F13. Titanium: the energy (Kroll process)
- The number(s): about 50 kWh per kg of titanium (180 MJ/kg) for Kroll extraction, "over 2 kg CO2 per kg". An older Indian DRDO paper gives about 25 kWh per kg of sponge (search snippet only). The two disagree by 2 times.
- Source: JOM 2017 review, Springer, https://link.springer.com/doi/10.1007/s11837-017-2664-4 (could not open: cookie-handshake redirect); DRDO Defence Science Journal paper (could not open: host not found)
- Quotation: "energy consumption: ca. 50 kWh/kg Ti" (taken from a search-result snippet; source page not read)
- Read: 2026-11-12 (snippet only)
- Confidence: low (not read at source).
- Use for the ship: titanium costs 25 to 50 kWh/kg and needs chlorine, magnesium (recycled in the process) and carbon; that is 5 to 10 times steel per kg. Use it only for small critical parts.

### F14. Carbon fibre: the energy
- The number(s): published totals span 262 to 1,132 MJ/kg of fibre (73 to 314 kWh/kg). Japanese industry life-cycle inventory: 318.2 MJ/kg process energy plus 32.1 MJ/kg feedstock energy = 350.2 MJ/kg. Review ranges: 286 to 1,132 MJ/kg (Chen et al. meta-analysis); 262.3 to 497.9 MJ/kg (Prenzel et al., Polymers 2023).
- Source: JCMA 2022 life-cycle press release, https://carbonfiber.gr.jp/english/tech/pdf/LCI-of-CF-JCMA-2022-PRESSRELEASE.pdf (page text not obtained); reviews seen in search results only
- Quotation: paraphrase, no exact quotation (search snippet only)
- Read: 2026-11-12 (snippet only)
- Confidence: low to medium (several independent studies agree on the order of magnitude, none opened).
- Use for the ship: carbon fibre costs about 300 MJ/kg (80 kWh/kg), about 15 times steel per kg. It is made from polyacrylonitrile, which comes from propylene and ammonia, so it needs carbon and nitrogen feedstocks (a standard route; not separately sourced). Treat it as a rare, small-volume material in the early ship.

### F15. Basalt fibre: the energy
- The number(s): specific energy to make a homogeneous basalt melt: 1.1 kWh/kg or less in an electromagnetic (lab) reactor; 3 to 7 kWh/kg in slag-type cupola and ore furnaces; 2 to 4.5 kWh/kg for induction heating. That is 4 to 25 MJ/kg, which brackets steel's 10 to 24 MJ/kg. Wikipedia: melts at about 1,500 C with essentially no additives (lead only).
- Source: Journal of Engineering Physics and Thermophysics, vol. 92 no. 1, January 2019, "Technology of electric melting of basalt for obtaining mineral fiber", https://link.springer.com/10.1007%2Fs10891-019-01929-3 (could not open: cookie-handshake redirect; read via a search snippet)
- Quotation: "does not exceed 1.1 kW·h/kg" (snippet paraphrase; full sentence not seen)
- Read: 2026-11-12 (snippet only)
- Confidence: low. Lab claim plus snippet; no cradle-to-gate life-cycle number found.
- Use for the ship: basalt fibre is the best fit for a colony: only crushed basalt and heat (one input, no carbon or nitrogen), roughly the energy of steel per kg, with 3,000 MPa strength (F8).

### F16. NEGATIVE: the University of Bath ICE embodied-energy tables are discontinued
- The number(s): none. The ICE database published embodied energy (MJ/kg) in its 2008 and 2011 versions; "From 2019, embodied energy factors were no longer included" and titanium is no longer covered in the current version.
- Source: Circular Ecology ICE database FAQ, https://circularecology.com/?p=1902 (search snippet; page not opened); Hammond and Jones, Proceedings of the ICE Energy 161, May 2008, pp. 87 to 98
- Quotation: "From 2019, embodied energy factors were no longer included" (snippet)
- Read: 2026-11-12 (snippet only)
- Confidence: medium.
- Use for the ship: use worldsteel, the WEF/IAI figure and the reviews above instead; do not cite ICE for current numbers.

## 1C. Design rules for allowable stress

### F17. ASME Section VIII Division 1 allowable stress
- The number(s): allowable stress is the LESSER of tensile strength / 3.5 and two-thirds of yield strength at room temperature (creep also at high temperature). Division 2 uses 2.4 instead of 3.5 on tensile. Bolting uses tensile / 4.
- Source: summary on Ward Vessel and Exchanger "Allowable Stress", https://wardvesselandexchanger.com/allowable-stress/ (read as a search snippet only); ASME BPVC Section II Part D is paywalled and not read
- Quotation: "the lesser of tensile strength divided by 3.5, or 2/3 of yield strength" (snippet)
- Read: 2026-11-12 (snippet only)
- Confidence: medium (this is the widely published rule, but I did not read the Code).
- Use for the ship: for a 700 MPa yield, 780 MPa-tensile steel: 2/3 yield = 467 MPa but tensile / 3.5 = 223 MPa, so the TENSILE rule governs in Division 1 (about 220 MPa). Division 2's tensile / 2.4 gives about 325 MPa. Use about 220 to 325 MPa allowable for S690-class steel in a code-compliant vessel.

### F18. NASA-STD-5001B: factors of safety
- The number(s): metallic structure: ultimate design factor 1.4; yield design factor 1.0 (prototype) or 1.25 (protoflight). HABITABLE MODULES, internal pressure only: yield factor 1.65, ultimate factor 2.0, proof test 1.5; negative pressure difference: ultimate 1.5 (2.0 if verified by analysis only). Habitable modules are explicitly not treated as pressure vessels.
- Source: NASA-STD-5001B with Change 3, Structural Design and Test Factors of Safety for Spaceflight Hardware, NASA, https://standards.nasa.gov/sites/default/files/standards/NASA/B-w/CHANGE-3/3/2022-10-24-NASA-STD-5001B-w-Change-3-Approved.pdf, Change 3 approved 2022-10-24 (file name)
- Quotation: "Habitable modules are not considered "pressure vessels.""; Table 5 row: "Internal pressure only 1.65 2.0 1.5"
- Read: 2026-11-12
- Confidence: high (the standard's own text, extracted from the PDF).
- Use for the ship: design the drum hull so that yield is not reached below 1.65 times working load and fracture is not reached below 2.0 times. So allowable stress is the LOWER of yield / 1.65 and ultimate / 2.0: for S690QL at 690 yield and about 770 ultimate, that is min(418, 385) = about 385 MPa (my arithmetic). Metallic pressure VESSELS (not habitable modules) go to ANSI/AIAA S-080 per the standard. NASA-STD-5009, -5019 not read (gap).

## 1D. Largest plates, plate bending, how big cylinders are made

### F19. Dillinger (Germany), the widest and heaviest Western plate
- The number(s): maximum plate length 28 m; maximum plate weight up to 52 t (flyer of 42 t, 52 t in special cases for the 2024 version, per a search result); cut-to-shape plates ex mill 40 to 450 mm thick, up to 5,200 mm wide, 2,000 to 25,000 mm long, unit weight 45 t; shot-blasted plate up to 200 mm x 4,500 mm x 28 m, 38 t.
- Source: Dillinger Facts and Figures 2025, https://en.dillinger.de/app/uploads/sites/5/2026/06/Web_Daten_und_Fakten_2025_Flyer_DH_EN.pdf, 2025; and "Heavy steel plate from stock and steel services" (2024-09), https://fr.dillinger.de/app/uploads/2022/10/WEB_Heavy_steel_plate_from_stock_and_steel_services_2024_09_E.pdf
- Quotation: "Max. plate length 28 m  Max. plate weight up to 52 t"
- Read: 2026-11-12
- Confidence: high for 28 m and 52 t; medium for the cut-to-shape table (its columns came out interleaved; the figures match a search summary).
- Use for the ship: plates for the drum: 28 m x 5 m x up to 0.3 m, about 40 to 50 t each. Fits a field-welded drum made of thousands of such plates; thickness well above any hull need.

### F20. voestalpine (Austria): ordinary structural heavy plate
- The number(s): thickness 8 to 200 mm (up to 300 mm on request); width 3,950 mm max; length 18,700 mm (24,000 mm on request); pressure-vessel plate unit weight 20 t max.
- Source: voestalpine heavy plates of general structural steels, https://voestalpine.com/stahl/en/Products/Heavy-plates/Heavy-plates-made-of-general-structural-steels, undated
- Quotation: "3,950 mm max. (dependent on thickness and steel grade)"
- Read: 2026-11-12
- Confidence: high.
- Use for the ship: typical European mills stop at about 4 m wide and 19 to 24 m long; Dillinger is the high end of the West.

### F21. The widest mill in the world: Henan Iron and Steel (China)
- REMOVED 2026-11-12: news.metal.com disallows ClaudeBot. Its numbers are not used anywhere in this research.

### F22. Plate bending rolls
- The number(s): Faccin HAV three-roll with 5,000-tonne force "able to bend plates over 300 mm"; Faccin's general catalogue is reported to rate the HAV up to 350 mm (snippet only); Faccin 4HEL four-roll 5 to 150 mm thick, 1.5 to 18 m long; Davi four-roll 4,000 mm wide x 145 mm thick for offshore monopiles (snippet only); Haeusler 4-roll EVO machines list roll widths about 2.1 to 4.1 m (its own 1986 delivery was said to bend over 200 mm); typical wind-tower cans 3 to 7.5 m diameter from plate 18 to 80 mm, monopiles 5 to 8 m and 40 to 140 mm (The Fabricator, snippet).
- Source: OGN news, "Faccin delivers giant plate roll machine to Saudi firm", https://ognnews.com/Article/45076/Faccin_delivers_giant_plate_roll_machine_to_Saudi_firm, 5 May 2021; Haeusler EVO page, https://haeusler.com/en/plate-bending-machines/evo/, undated. The Fabricator's Davi article returned 403 (not read).
- Quotation: "able to bend plates over 300 mm" (the article does not say explicitly that this is thickness; presumably it is)
- Read: 2026-11-12
- Confidence: low to medium: press articles and snippets; no single maker's data sheet states "the largest roll ever built".
- Use for the ship: real rolls reach about 150 mm (cold, four-roll, 4 m wide) and about 300 mm (three-roll, very large press). A drum needs far less, see the next finding.

### F23. A 250 m radius drum barely needs bending (my arithmetic, no outside source)
- The number(s): a plate's outer-fibre bending strain is about thickness / (2 x radius). At R = 250 m: 100 mm plate gives 0.02 %; 300 mm plate gives 0.06 %. S690-class steel only yields at about 0.33 % strain (700 MPa / 210 GPa). A 5 m wide plate curved to R = 250 m has a mid-span rise (sagitta) of 5^2 / (8 x 250) = 12.5 mm.
- Source: none; first-principles arithmetic from F2 and the standard modulus of steel
- Quotation: not applicable
- Read: 2026-11-12
- Confidence: high on the arithmetic, since it depends on no unproven number.
- Use for the ship: the plates can be bent elastically (pulled into shape on a jig, or only lightly pressed), so no giant rolling mill is needed in orbit. Roll-then-weld versus flat-and-weld hardly differs at this radius, and either way the welding, not the bending, is the hard job.

### F24. How large cylinders are made today (API 650, wind towers)
- The number(s): API 650 tank shell plates are limited to 45 mm unless the standard says otherwise; common plate width 2,438 mm (8 ft); shell courses are built from curved plates welded in vertical and horizontal joints (not confirmed by the sources I read). Wind-tower and monopile "cans": plates rolled to a cylinder, closed with one long seam, then cans welded end to end (The Fabricator snippet).
- Source: forum quotations of API 650 section 4.2.1.4 (Eng-Tips, search snippet, unopened); API 650 itself not read (paywalled)
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12 (snippet only)
- Confidence: low.
- Use for the ship: the proven method for a big cylinder is "roll plates, weld a ring, weld rings together". For this drum that means rings of curved plate welded circumferentially along the 2 km length.

### F25. Welding in space: history (Welding Journal / NASA copy)
- The number(s): 1969 Soyuz 6 (Shonin, Kubasov), automated battery-powered "Vulkan" unit using electron beam, low-pressure plasma arc and consumable-electrode arc welding, inside the capsule; 1973 Skylab experiment M551 (electron beam, with brazing M552), Skylab 2 commander Conrad ran it; 1984 Salyut 7 (Savitskaya, Dzhanibekov) the first welding in OPEN space, a 3.5 hour spacewalk with a hand tool designed by the E.O. Paton Welding Institute in Kyiv; repeated 1986 (Solovyov and Kizim) to weld truss on Salyut's outside. Returned samples had mechanical properties comparable to ground welds.
- Source: "The Space Economy: A New Frontier for the Welding Industry", Jeffrey W. Sowards (NASA Marshall), Welding Journal, 2023 (NTRS 20230011078), https://ntrs.nasa.gov/api/citations/20230011078/downloads/NASA%2020230011078.pdf
- Quotation: "the first space welding and cutting of metals in their Soyuz 6 capsule using the automated and battery-powered Vulkan unit"; "mechanical properties of aluminum and stainless steel welds made in a space station were similar to those made on Earth"
- Read: 2026-11-12
- Confidence: high that these events are credited in a NASA-hosted article. Caveat: a Kyiv university page (search snippet, not opened) says the 1969 device malfunctioned and nearly burned through the outer shell; the sources differ on "first" (Soyuz 6 first automated, Salyut 7 first manual).
- Use for the ship: electron-beam welding works in vacuum (it is the one process that wants a vacuum), and was shown on aluminium, stainless steel and titanium with ground-equivalent strength. For thick steel plate the proven industrial processes are arc welding under shielding gas; the vacuum of space helps electron beam, hurts arc.

### F26. Welding large rotating stations: NASA / industry plans
- The number(s): NASA and industry plans to unfurl and weld hexagonal and pentagonal panels into a pressurised 20 m diameter sphere in orbit; commercial wheel-station plans use welding to reach diameters near 500 m.
- Source: same Welding Journal article as F25
- Quotation: "weld a set of hexagonal and pentagonal panels in low Earth orbit, creating a pressurized and habitable 20-m-diameter spherical structure"
- Read: 2026-11-12
- Confidence: medium (a plan, not a built thing).
- Use for the ship: nothing built so far is larger than a few metres; a drum 500 m wide would be far beyond precedent. Say so on the ship page.

### F27. Skylab M551 result (negative of a different kind)
- The number(s): none beyond the existence of a rotating-disk electron-beam experiment run in June 1973; tantalum sample showed erased grain-flow vortices compared with the ground sample (search snippet only). I found no account of an astronaut-run structural weld in space beyond F25.
- Source: NASA TM X-64960, Poorman, "Skylab M551 Metals Melting Experiment", 1975, https://ntrs.nasa.gov/citations/19760003076 (not opened)
- Quotation: not read
- Read: 2026-11-12 (snippet only)
- Confidence: low.
- Use for the ship: no more needed.

# PART 2: RADIATION SHIELDING WITH WATER

### F28. How much water, by thickness: the OLTARIS table (the most useful single source)
- The number(s): yearly dose in free space (deep space, outside any magnetic field), effective dose equivalent for a female body model, behind water shielding of each mass per square metre:
  - none (0): 462 mSv/yr
  - 1 t/m2 (100 g/cm2, 1 m of water): 200
  - 2 t/m2: 147
  - 3 t/m2: 101
  - 4 t/m2: 67
  - 5 t/m2: 43
  - 6 t/m2: 26.5
  - 7 t/m2: 16.1
  (polyethylene is a little better: 7 t/m2 gives 12.3). Unit conversion: 1 t/m2 = 100 g/cm2 = 1 m of liquid water. Lunar regolith needs 10 to 11 t/m2 for 20 mSv.
- The authors' conclusion: "6-7 tonnes of water or polyethylene radiation shielding per square meter of hull is sufficient in deep space" to meet 20 mSv/yr.
- Source: "Orbital Space Settlement Radiation Shielding", Al Globus (San Jose State University) and Joe Strout, NSS Space Settlement Journal, April 2017 (Table 2, computed with NASA's OLTARIS tool), https://NSS.org/wp-content/uploads/2018/01/NSS-JOURNAL-Orbital-Space-Settlement-Radiation-Shielding.pdf
- Quotation: "6-7 tonnes of water or polyethylene radiation shielding per square meter of hull is sufficient in deep space"
- Read: 2026-11-12
- Confidence: medium. NASA's calculator, but the paper is from an advocacy society's journal (not peer-reviewed in the usual sense; the authors call their conclusions preliminary), the solar condition was not stated in the part I read, and the table shows effective dose, not a certified limit.
- Use for the ship: this answers "20, 50, 100+ g/cm2": the dose falls about 2 times at 100 g/cm2, about 10 times at 500 g/cm2 and 29 times at 700 g/cm2. So 20 or 50 g/cm2 are nowhere near enough for a home. Falls by roughly 0.6 per extra tonne past 5 t/m2 (my reading of the ratios 0.64, 0.62, 0.61).

### F29. NASA: GCR shielding falls slowly and adds secondaries; the 20 g/cm2 region
- The number(s): in NASA Langley's modelling, the dose equivalent behind aluminium shielding reaches a shallow minimum near 20 to 40 g/cm2, beyond which more shielding gives no gain; for polyethylene (a water-like hydrogen material) it "flattens out after approximately 20 g/cm2". Dose increases past the minimum "due to the buildup of neutrons".
- Source: "Early Results from the Advanced Radiation Protection Thick GCR Shielding Project", Norman, Clowdsley, Slaba et al., NASA Langley, 2017, https://ntrs.nasa.gov/api/citations/20170005580/downloads/20170005580.pdf
- Quotation: "the dose equivalent decreases until approximately 40 g/cm2 of aluminum shielding"; "simply flattens out after approximately 20 g/cm2"
- Read: 2026-11-12
- Confidence: high that NASA reports this; but the geometry is a thin slab with equal shielding in front and behind, a spacecraft case, not a thick hull. It conflicts in part with F28 (the OLTARIS table keeps falling out to 700 g/cm2): see the next line.
- Use for the ship: both are NASA's code. For heavy, thick, one-sided shielding (our case) F28 is the closer match, but the flat-slab result is why small amounts of shielding do little against GCR. Treat any value over 100 g/cm2 as a model estimate, not a measured one.

### F30. NASA storm shelter guidance: water walls for solar particle events
- The number(s): "at least 4 inches" of water (10.2 cm, about 10 g/cm2) for effective SPE protection; 4.3 in (10.8 g/cm2) of water gave a 50 % dose reduction; 8.1 in (20.7 g/cm2) of water needed for 50 % reduction at 68 % confidence in the crew-quarters design; 3.7 in (9.3 g/cm2) for another case. The NASA design guideline of about 20 g/cm2 (the figure you asked about) came from a search snippet only (see below).
- Source: "Design of Two RadWorks Storm Shelters for Solar Particle Event Shielding", Simon, Cerro, Latorella, Clowdsley, Watson et al., NASA Langley (cites AIAA 2013-5435), https://ntrs.nasa.gov/api/citations/20150001237/downloads/20150001237.pdf, undated in the extract
- Quotation: "The water wall should be at least 4 inches thick to provide effective SPE radiation protection in the deep space environment"
- Read: 2026-11-12
- Confidence: high for the quoted values (NASA's design paper). The "20 g/cm2 in all areas" sentence is from a search-result snippet (not opened), so medium.
- Use for the ship: a storm shelter needs about 10 to 25 g/cm2 of water (10 to 25 cm) all round; the standard design goal is about 20 g/cm2. The RadWorks paper says solar particle events are "made up primarily of medium-energy protons", so material shielding works for them, while GCR remains "an open issue".

### F31. Free-space dose rate unshielded (measured)
- The number(s): the Mars Science Laboratory's RAD instrument measured an average GCR dose equivalent of 1.8 mSv per day in cruise, about 657 mSv per year; round trip to Mars 0.66 +/- 0.12 Sv; 253-day cruise; only about 5 % from solar particles (quiet Sun). The NSS table's unshielded figure (462 mSv/yr effective dose) and this one differ because one is effective dose for a body model and the other a measured dose equivalent behind the spacecraft's own shielding.
- Source: SwRI press release of 30 May 2013 on Zeitlin et al., Science 340, 1080 to 1084 (2013), https://www.swri.org/press-release/swri-led-team-calculates-radiation-exposure-associated-trip-mars; abstract at https://elib.dlr.de/83619/
- Quotation: "The RAD data show an average GCR dose equivalent rate of 1.8 milliSieverts per day in cruise."
- Read: 2026-11-12
- Confidence: high (peer-reviewed measurement; I read the lab's press release and a repository record, not Science itself).
- Use for the ship: unshielded deep space is about 460 to 660 mSv per year, so 600 mSv (the career limit) is gone in about a year. Both numbers give the same order.

### F32. NASA career limit
- The number(s): career effective dose from spaceflight radiation must stay below 600 mSv (for all ages and sexes, applied with a 35-year-old female model); short-term limit 250 mSv per SPE.
- Source: NASA-STD-3001 Technical Brief OCHMO-TB-020 "Design for Ionizing Radiation Protection", Rev E, 12/27/2022, https://www.nasa.gov/wp-content/uploads/2023/03/radiation-protection-technical-brief-ochmo.pdf
- Quotation: "total career effective radiation dose due to space flight radiation exposure shall be less than 600 mSv"
- Read: 2026-11-12
- Confidence: high for the value; the brief cites NASA-STD-3001 Volume 1 Rev B and I could not confirm the Rev C wording itself (a search result mentioned Rev C; unconfirmed).
- Use for the ship: 600 mSv is a mission-career limit for adult astronauts, not a standard for a lifelong home with children; use it as the "never exceed" outer line, and ICRP's public limit (next) as the home target.

### F33. Public dose limit, 1 mSv per year
- The number(s): 1 mSv in a year for members of the public (above natural background); a higher single year is allowed if the 5-year average stays at 1 mSv per year (ICRP). US NRC: 0.1 rem = 1 mSv in a year.
- Source: ICRPedia "Dose limits" (quoting ICRP Publication 103, Table 6), https://icrpaedia.org/Dose_limits, last edited 20 June 2019; 10 CFR 20.1301(a)(1), https://www.law.cornell.edu/cfr/text/10/20.1301
- Quotation: "1 mSv in a year" (ICRPedia table); "does not exceed 0.1 rem (1 mSv) in a year" (10 CFR 20.1301(a)(1))
- Read: 2026-11-12
- Confidence: high (regulation text and the ICRP summary). I did not open ICRP 103 itself.
- Use for the ship: the Globus and Strout paper argues for a looser 20 mSv per year for settlers (the occupational limit), and notes Earth's average background is 3.1 mSv/yr (US) and 2.4 mSv/yr (world). A target of 1 to 5 mSv per year places the ship at ordinary Earth background.

### F34. Water ice
- The number(s): density of ice at 0 C is 0.917 g/cm3, "about 8% less" than water; thickness for the same areal density is 1.09 times that of liquid water (my arithmetic: 1 / 0.917); water expands about 9 % in volume on freezing. USGS says ice's density "decreases by about 9 percent" (search snippet only; the USGS page timed out twice).
- Source: Introduction to Ocean Sciences, section 5.9 Ice Formation (Segar), https://geo.libretexts.org/Courses/American_Meteorological_Society/Introduction_to_Ocean_Sciences_(Segar)/05%3A_Water_and_Seawater/5.09%3A_Ice_Formation, undated
- Quotation: "Ice at 0°C has a density of 0.917 g•cm–3, about 8% less than the density of water of the same temperature."
- Read: 2026-11-12
- Confidence: high for 0.917 (textbook); the 9 percent is the consistent reciprocal (1 / 0.917 = 1.090).
- Use for the ship: size the sealed cells for 9 % expansion, or keep the water liquid with heating.

### F35. Does ice shield as well as water, per kg?
- The number(s): no peer-reviewed comparison found. A 2026 ESA workshop abstract proposes ice for Mars habitats and says ice offers "reduced secondary particle production" (a potential benefit, preliminary simulation, not quantified).
- Source: "Radiation Protection for Martian Habitats Using Water Ice in Inflatable Structure", Graham, Chatzipapas, van Ligtenberg, European Space Radiation Shielding Workshop, ESA/ESTEC, 17 June 2026, https://indico.esa.int/event/659/contributions/12437/
- Quotation: "Ice offers benefits, such as the potential for in-situ resource utilization and reduced secondary particle production."
- Read: 2026-11-12
- Confidence: low (an unreviewed 5-minute workshop pitch).
- Use for the ship: by physics the same areal density of water and ice has the same hydrogen content (both H2O), so per kg they should shield the same; I found no source, so label this "expected, not sourced". Shielding depends on mass per area, so ice needs 9 % more thickness for the same mass.

### F36. Old project figure, for reference
- The number(s): the Globus and Strout paper repeats the 1970s estimate "roughly 4.5 tonnes per square meter of hull" for 5 mSv/yr at L5 and says it is "quite low": their OLTARIS runs say water needs about 10 to 12 t/m2 for 5 mSv/yr by my extrapolation (below).
- Source: same NSS paper, https://NSS.org/wp-content/uploads/2018/01/NSS-JOURNAL-Orbital-Space-Settlement-Radiation-Shielding.pdf
- Quotation: "Unfortunately, the 4.5 tonnes per square meter of hull estimate is quite low."
- Read: 2026-11-12
- Confidence: medium.
- Use for the ship: this reconciles with the project's NASA SP-413 value (4.5 t/m2): the table shows that figure gives about 60 mSv/yr in water (between 43 at 5 t and 67 at 4 t), not 5.

# RECOMMENDATIONS

(a) Permanent home at 1 to 5 mSv per year, water on a non-spinning shell
- Extending the F28 table with its own decay (about x0.6 per extra tonne per m2): 8 t/m2 is about 10 mSv/yr; 9 t about 6; 10 t about 3.5; 11 t about 2.1; 12 t about 1.3; 13 t about 0.8. This is MY extrapolation beyond the last row (7 t/m2), not a published value.
- Recommend 10 m of water, 10,000 kg/m2 (1,000 g/cm2), for about 3.5 mSv/yr; use 12 m (12 t/m2) to reach about 1 to 1.5 mSv/yr. 6 to 7 m (6 to 7 t/m2) is the minimum that meets the looser 20 mSv/yr settler limit.
- As ice the depth is 1.09 times greater: 10.9 m. The hull, air, soil, crops and other mass already between people and space count too, and cut the water needed (not quantified in my sources).
- Scale: the drum's side surface (radius 250 m, length 2,000 m) is about 3.14 million m2, so 10 t/m2 is about 31 million tonnes of water over the side alone; the two end caps add about 0.4 million m2. This shows a ship-wide water jacket is a huge mass; consider shielding only the inhabited zones, or deep rock/regolith layers instead (11 t/m2 of regolith gives only 14.5 mSv/yr per the same table, so water is about 1.4 times better per kg).

(b) Storm shelter only
- 20 to 25 g/cm2 of water all round the shelter: 0.20 to 0.25 t/m2, i.e. 20 to 25 cm of water (22 to 27 cm of ice). NASA's RadWorks design needed 10 to 21 g/cm2 for a 50 % cut in a solar particle event dose and set at least 4 inches (10 cm) as the floor. A shelter in the middle of an already shielded home needs only the small extra from the walls.

# Could not read
- https://www.usgs.gov/water-science-school/science/water-density: fetch timed out twice (60 s).
- https://www.nature.com/articles/s41598-022-17079-1: redirected to a cookie-authorize page; not worked around.
- https://link.springer.com/doi/10.1007/s11837-017-2664-4 and https://link.springer.com/10.1007%2Fs10891-019-01929-3: Springer cookie-authorize redirect; not worked around.
- https://pmc.ncbi.nlm.nih.gov/articles/PMC9916691 and https://pubmed.ncbi.nlm.nih.gov/38201677/: bot-check / cookie page.
- https://core.ac.uk/download/72055254.pdf: 403.
- https://publications.drdo.gov.in/...: host not found.
- https://mdpi.com/2075-163X/15/1/17: 403. https://www.scientific.net/KEM.1051.9.pdf: 403.
- https://www.thefabricator.com/.../davi-delivers-large-capacity-plate-roll-to-wind-tower-manufacturer: 403.
- https://ssed.gsfc.nasa.gov/LEADER/Abstracts/jordan_2023_esf_abstract.pdf: host not found.
- https://www.nasa.gov/wp-content/uploads/static/history/alsj/WOTM/NASA_TP_3079.pdf: 404.
- https://worldsteel.org/wp-content/uploads/Sustainability-Indicators-2025.pdf: 404.
- https://www.ecfr.gov/...: redirected to a block page (used Cornell LII for 10 CFR 20.1301).
- https://ntrs.nasa.gov/api/citations/20070032702/downloads/20070032702.pdf (Lin, 2007): scanned, OCR too poor to use.
- https://carbonfiber.gr.jp/...LCI...pdf (JCMA): page returned only menus; figures from a search snippet.
- Robots: nasa.gov, ntrs.nasa.gov, ssab.com, nrc.gov, dillinger.de, ecfr.gov allow general crawlers. I did not fetch robots.txt for every host (SwRI, Hexcel, UPMET, ATI, ArcelorMittal, voestalpine, Mafic/Knowde, Cornell LII, icrpaedia, libretexts, worldsteel, metal.com, ognnews, haeusler, WEF, NSS, ESA indico); none refused a fetch.

# Negative findings and gaps
- No page I read gives steel's modulus and density, 7075's modulus, 2219's density or modulus, or the strength of a basalt-fibre COMPOSITE. (Textbook steel: 210 GPa, 7,850 kg/m3; unsourced here.)
- ICE (University of Bath) no longer publishes embodied energy since 2019 (F16); titanium is dropped from its current version.
- No clean peer-reviewed cradle-to-gate energy for basalt fibre, and no source I could open for titanium or carbon-fibre energy (F13 to F15 are snippet-level).
- ASME Section VIII Division 1 (paywalled) and API 650 were not read; F17 and F24 rest on secondary summaries. NASA-STD-5009 and NASA-STD-5019 were not read; NASA-STD-5001B's tables cover design factors instead (F18).
- No published ice-versus-water shielding comparison per kg; only the F35 workshop abstract.
- No published figure for water depths above 7 t/m2; my 8 to 13 t/m2 values are extrapolated.
- NASA's own evidence reports (Human Research Program) and NCRP reports were not opened; the NSS-journal table (F28) uses NASA's OLTARIS code but is not a NASA publication, and NASA's own Norman et al. paper (F29) points to a flatter dose curve for thin slabs. Which governs for a thick one-sided water jacket was not resolved.
- The 600 mSv value is from the Rev E (2022) technical brief that cites Rev B of NASA-STD-3001 Volume 1; the Rev C text was not read.
- The search budget ran out before a basalt composite search and any further GCR-vs-water-depth NASA table.

# Summary table
| Question | Recommended number | Findings | Confidence |
|---|---|---|---|
| S355 yield / tensile | 355 down to 295 MPa (thick) / 470 to 630 MPa | F1 | medium |
| S690QL / A514 yield | 690 MPa (thin), 630 (100 to 200 mm); tensile 710 to 940 | F2, F3 | high |
| Ti-6Al-4V | 827 MPa yield, 903 tensile, 114 GPa, 4.43 g/cm3 | F4 | medium |
| 5083-H321 | 228 MPa yield, 317 tensile, 71 GPa, 2.66 g/cm3 | F5 | medium |
| 2219 | 317 MPa yield (T851), 427 tensile, weldable | F6 | medium |
| 7075-T6 | about 430 to 475 MPa yield, 510 to 540 tensile, 2.80 g/cm3 | F7 | medium |
| Basalt fibre | 3,000 MPa, 88 GPa, 2.63 g/cm3 (composite: not found) | F8 | medium |
| Carbon fibre (IM7) | 5,670 MPa, 276 GPa, 1.78; composite 2,723 MPa, 164 GPa | F9 | high |
| Steel energy | 24 MJ/kg from ore, 10 MJ/kg from scrap; H2 route 3.5 kWh/kg electricity | F10, F11 | high / medium |
| Aluminium energy | about 70 MJ/kg (19 kWh/kg); recycled 5 % | F12 | medium |
| Titanium energy | 25 to 50 kWh/kg | F13 | low |
| Carbon fibre energy | about 300 MJ/kg (range 260 to 1,130) | F14 | low to medium |
| Basalt fibre energy | about 1 to 7 kWh/kg of melt (4 to 25 MJ/kg) | F15 | low |
| ASME VIII-1 allowable | lesser of Su/3.5 and 2/3 Sy (Div. 2: Su/2.4) | F17 | medium |
| NASA factors, habitable module | yield 1.65, ultimate 2.0, proof 1.5 (internal pressure) | F18 | high |
| Largest plates | 5.3 m x 0.3 m (Henan); Dillinger 5.2 m wide, 28 m long, 52 t; voestalpine 3.95 m, 18.7 m | F19, F20, F21 | medium to high |
| Largest rolls | about 150 mm four-roll; about 300 mm three-roll | F22 | low to medium |
| Bending at R = 250 m | strain 0.02 to 0.06 %: elastic, no big rolls needed | F23 | high (arithmetic) |
| Welding in space | Vulkan 1969 (Soyuz 6), Skylab 1973, URI/Paton hand tool 1984 on Salyut 7 | F25 | high |
| Free-space GCR dose | 1.8 mSv/day (about 650 mSv/yr) measured; 462 mSv/yr modelled | F31, F28 | high / medium |
| Limits | 600 mSv career (NASA); 1 mSv/yr public (ICRP, NRC) | F32, F33 | high |
| Water vs dose | 100 g/cm2 gives 200; 500 gives 43; 700 gives 16.1 mSv/yr | F28 | medium |
| Home at 1 to 5 mSv/yr | 10 to 12 m of water, 10 to 12 t/m2 (extrapolated; 6 to 7 t/m2 for 20 mSv/yr) | F28, F36 | medium to low |
| Storm shelter | 20 to 25 g/cm2 = 20 to 25 cm of water (min 10 cm) | F30 | high |
| Ice | 0.917 g/cm3, 9 % expansion, same shielding per kg (expected, unsourced) | F34, F35 | high / low |
