# Helper notes G: Power per person, reactors, fuel, siting, the catastrophe case, photovoltaics (README section 8)

Part of the [mothership sizes research](README.md) (2026-11-13). A research helper wrote these
notes on 2026-11-12 from the sources it could read; the README carries the answers and the
judgements. After the helper finished, every cited host's robots.txt was checked: findings
read from a page that refuses Claude's readers are marked REMOVED and their numbers are not used.
Quotations were spot-checked against their sources where the README says so; the rest are the
helper's readings, with the helper's own confidence. The helper's own robots notes and summary
table were written before that check: where they name a REMOVED finding or say a host allowed
reading, the check below the README's "How this was read" section is the one that holds.


Read-only research. Robots.txt was checked first for energy.gov, nrc.gov, ecfr.gov, nasa.gov, iaea.org and osha.gov (none block the paths read). nrel.gov did not resolve (the lab is now at nlr.gov; read there, robots not fetched for nlr.gov, the page loaded without a challenge).

Marking: "EXACT" means the quotation was returned from the page or PDF text. "SNIPPET" means the fact came from a search-result summary and the page itself was not read; those are Confidence medium or low and the ship team should re-read the page before quoting. My own arithmetic is labelled "calc".

---

## 1. Power per person

### G1. LED grow-light efficacy floor (DLC list)
- The number(s): minimum 2.30 umol/J (DLC Horticultural Technical Requirements V3.0, effective March 2023). V4.0 (applications from 18 April 2025) raises that by 8.7%, so about 2.50 umol/J (calc: 2.30 x 1.087 = 2.50). The project's 2.8 umol/J sits above the floor.
- Source: DesignLights Consortium, "Horticultural Technical Requirements V3.0" (PDF) and DLC horticultural technical requirements page (V4.0 percentage), https://www.designlights.org/dlc_hort-technical-requirements-v3-0_final_05132024/ and https://www.designlights.org/horticultural-lighting/technical-requirements/ , source's own date: V3.0 effective March 2023; V4.0 effective April 18, 2025
- Quotation (V3.0, EXACT from the PDF text): "All products must have a PPE of at least 2.30 µmol · J-1" (the PDF text shows the micro sign garbled; the number is clear). V4.0 (EXACT, DLC page): "increases the photosynthetic photon efficacy (PPE) threshold by 8.7%"
- Read: 2026-11-12
- Confidence: high for 2.30; medium for 2.50 (the DLC page gives only the percentage; a trade site says 2.5 but I could not read an explicit DLC sentence)
- Use for the ship: 2.8 umol/J is a plausible "best qualified fixture" value today; 2.5 is the listing floor. Keep 2.8 for the budget but say a 2.5 fixture costs 12% more electricity.

### G2. How high LED efficacy can go (best listed, and the ceiling)
- The number(s): the DLC list ranged 1.81 to 3.69 umol/J in early 2022, with only about 6% of products at 3.0 or more. Theoretical efficacy limits of about 3.4 umol/J (white + red) and 4.1 umol/J (blue + red), about 10% lower once sealed against water. Double-ended HPS: about 1.7 umol/J.
- Source: Michigan State University Extension (Runkle), "LED efficacy" / "Update on LED efficacy" PDFs, https://www.canr.msu.edu/floriculture/uploads/files/LED efficacy.pdf , 2022 and 2024 (SNIPPET only, page not opened)
- Quotation: paraphrase, no exact quotation (search summary)
- Read: 2026-11-12
- Confidence: medium-low (university extension summary, read only as a search summary)
- Use for the ship: 2.8 is in the top tier today but below the 4.1 ceiling. No source found supports assuming more than about 3.4 for a sealed white-plus-red fixture. Keep 2.8; treat 3.4 as a 2040s upside.

### G3. LED wall-plug efficiency (what share of electricity becomes photons)
- The number(s): calc: one watt of 660 nm red light is about 5.5 umol/s and of 450 nm blue about 3.8 umol/s, so a red/blue mix is about 4.6 umol per radiant watt. 2.8 umol/J / 4.6 = about 60% wall-plug efficiency. This agrees with the README's "50 to 60%".
- Source: no DOE horticultural wall-plug figure found. DOE general SSL goals found only as a snippet: "30% (in 2025) and 55% (in 2035)" for wall-plug efficiency of SSL in general (OSTI technical report 3027503, https://www.osti.gov/biblio/3027503 , SNIPPET). A patent states red LEDs reach WPE "exceeding 50%" (US 10299441, not a DOE source).
- Read: 2026-11-12
- Confidence: medium for the calc (physics), low for any DOE statement
- Use for the ship: say plainly that 40% of the lamp electricity is heat at the lamp, and the other 60% becomes light that plants and walls turn into heat too. All 10.8 kW per person ends up as heat the ship must radiate away (see G30).

### G4. Energy per kg of vertical-farm produce
- The number(s): lettuce, plant factory: 247 kWh of electricity per kg DRY weight. Greenhouses for the same crop: 70, 111, 182 and 211 kWh per kg dry weight (Netherlands, UAE, Sweden without and with lighting). A modelling study in the NSF repository gives 6.2 to 16 kWh per kg FRESH lettuce (SNIPPET). Calc: a person eating about 0.6 kg dry food per day at 247 kWh/kg is 148 kWh/day = about 6.2 kW, so the project's 9.4 kW of lighting is about 1.5x that figure (headroom for staples that need more light than lettuce, and for non-food plants).
- Source: Graamans, van den Dobbelsteen, Meinen, Stanghellini, "Plant factories versus greenhouses: Comparison of resource use efficiency", Agricultural Systems 160:31-43, https://research.wur.nl/en/publications/plant-factories-versus-greenhouses-comparison-of-resource-use-eff , source's own date: 1 February 2018 (DOI 10.1016/j.agsy.2017.11.003)
- Quotation (EXACT, from the abstract as returned by the page): "requires an input of 247 kWhe in a plant factory, compared to 70, 111, 182 and 211 kWhe in greenhouses"
- Read: 2026-11-12
- Confidence: high for the abstract numbers (peer reviewed); the 0.6 kg/day figure is my assumption, not sourced
- Use for the ship: 9.4 kW of crop lighting per person is consistent with the best published plant-factory figure; do not cut it below about 6 kW per person without a better source. The same paper says plant factories beat greenhouses on water, CO2 and land, which is why the ship uses lamps (no sun).

### G5. Life support (non-crop) power per crew member
- The number(s): no single published ECLSS kW-per-person figure found. Subsystem pieces: Oxygen Generation System about 3.6 kW; urine and water processors about 0.8 kW; CO2 removal (CDRA) about 840 W for six crew (calc: about 140 W per person); water recovery ground test 743 W-hr/hr. Together on ISS these are about 5 kW for 6 crew, calc, so about 0.8 kW per person before cabin cooling (SNIPPET, from NASA ICES-type papers; not opened).
- Source: NASA ECLSS papers surfaced by search, e.g. https://ntrs.nasa.gov/api/citations/20090033097/downloads/20090033097.pdf (SNIPPET only)
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: low-medium
- Use for the ship: 10.8 - 9.4 = 1.4 kW per person for everything else is enough for life support at ISS-scale (about 0.8 kW) plus a little. It is tight once water, air, cooling pumps, appliances and industry are added, so Section 1's spare margin matters. (BVAD 2022, NASA TP-2015-218570 Rev2, NTRS 20210024855, exists but I could not read its power tables: see Could not read.)

### G6. Electricity per person in a modern country (for scale)
- The number(s): 2023, kWh per person per year: United States 12,710.8 (= 1.45 kW average); OECD members 7,635.5 (= 0.87 kW); World 3,577.5 (= 0.41 kW). The ship's 10.8 kW = 94,600 kWh/yr, which is 7.4 times the US average, and 26 times the world average (calc).
- Source: World Bank indicator EG.USE.ELEC.KH.PC "Electric power consumption (kWh per capita)" (built from IEA data), via https://api.worldbank.org/v2/country/USA;WLD;OED/indicator/EG.USE.ELEC.KH.PC?format=json , source's own date: data year 2023
- Quotation: table row read from the API: "United States | 2023 | 12,710.8" (EXACT value; the tool returned a table, not a sentence)
- Read: 2026-11-12
- Confidence: high (official statistical series)
- Use for the ship: 10.8 kW per person is an honest "everything is made on board and food is lit" figure, far above any country because no one outside the ship grows the food or makes the goods. A stat page can show: "US home-and-industry average 1.45 kW; ship 10.8 kW".

### G7. Industry aboard: electricity to make steel
- The number(s): scrap melted in an electric arc furnace: 400 to 500 kWh per tonne (one plant 427 kWh/t) (SNIPPET). Hydrogen direct-reduced iron plus EAF, all electricity including water electrolysis: 3.72 MWh per tonne liquid steel (Bhaskar et al.; SNIPPET); a second attribution (Vogl et al. 2018) gives 3.48 MWh/t but I could not confirm which paper owns which number. Calc: 1 tonne per person per year of new H2-route steel = 3,720 kWh/yr = 425 W continuous; 100 kg per person per year = 42 W.
- Source: Bhaskar, Assadi, Nikpey Somehsaraei, "Decarbonization of the Iron and Steel Industry with Direct Reduction of Iron Ore with Green Hydrogen", Energies, February 2020, https://doaj.org/article/86813bc9c8b04962bac6ac9909af84f1 (403 on fetch, SNIPPET only)
- Quotation: paraphrase, no exact quotation (search snippet: "3.72 MWh")
- Read: 2026-11-12
- Confidence: medium-low (secondary snippet; two papers disagree in the snippet)
- Use for the ship: industry per person is small next to lamps. Even 1 tonne of fresh steel per person per year is about 0.4 kW. Recycling scrap in an EAF is about 10 times cheaper than H2 ore reduction, so the ship should be a recycling economy.

---

## 2. Reactor sizes

(The IAEA 2024 booklet "Small Modular Reactors: Advances in SMR Developments 2024", IAEA/PAT/008, DOI 10.61092/iaea.3o4h-svum, exists but its PDF returned 402 Payment Required and the ARIS per-reactor PDFs returned 404, so figures below come from the makers, an operator/OKBM slide deck, WNN and DOE. Record in Could not read.)

### G8. KLT-40S (Akademik Lomonosov)
- The number(s): 35 MWe each; 150 MWt each (the 150 comes from search results, the 35 and the two-reactor plant are read on WNN). Refuelling "up to 3.5 years" (other sources: 2.5 to 3 years). Calc: 35/150 = 23% gross efficiency; the project's 60 MW net a pair is 20% of 300 MWt.
- Source: World Nuclear News, "Floating nuclear power plant set for first refuelling", https://world-nuclear-news.org/articles/floating-nuclear-power-plant-set-for-first-refuell , source's own date: not printed in the page text I got
- Quotation (EXACT): "there can be up to 3.5 years between refuellings"
- Read: 2026-11-12
- Confidence: medium (trade news site quoting the operator; no ARIS read; the 150 MWt not read on the page)
- Use for the ship: keep as the project's reference design, since it is in service. Vessel dimensions: no published figure found in the pages I could open.

### G9. RITM-200 family
- The number(s): RITM-200 plant: 2 reactors x 175 MWt, 2,200 t, overall 6 x 13.2 x 15.5 m (the pair as a unit). RITM-200N (land): 1 x 190 MWt, 1,500 t, overall 8.7 x 21 m, 55 MWe gross; design lifetime 495,000 h (about 56 years). Fuel: "cermet fuel enriched to below 20%"; no partial refuellings, fuel assemblies discharged after the design stored energy. Refuelling interval printed in the deck but the number did not come out of the PDF text (the OECD NEA table cell is empty in the text layer); a search summary says 5 years for the RITM-200N.
- Source: OKBM Afrikantov, "RITM-200N regulatory ... SNPP" presentation, OECD NEA Workshop on Light-Water SMRs, 2024, https://oecd-nea.org/mdep/events/LWSMMRWS_2024/presentations/S2_OKBM_RegulatoryregulationSNPP_RITM-200NRP.pdf , source's own date: 2024 workshop
- Quotation (EXACT, from the PDF text layer): "Fuel type: cermet fuel enriched to below 20%"
- Read: 2026-11-12
- Confidence: medium-high for the table (maker's own deck read from the PDF text); medium for 5 years (summary only)
- Use for the ship: RITM-200N is the best match for "one drum plant": 55 MWe from a 1,500 t, 8.7 x 21 m plant, enrichment below 20%. Note the one-reactor plant weight and volume are for a station with its own shielding.

### G10. NuScale (US460 / 77 MWe module)
- The number(s): 77 MWe per module (250 MWt), up to six modules = 462 MWe; NRC Standard Design Approval, May 2025. Module physical size: not found (the NRC letter was listed but not read).
- Source: US DOE Office of Nuclear Energy, "NRC Approves NuScale Power's Uprated Small Modular Reactor Design", https://www.energy.gov/ne/articles/nrc-approves-nuscale-powers-uprated-small-modular-reactor-design , source's own date: May 30, 2025
- Quotation (EXACT): "77 MWe for each power module"
- Read: 2026-11-12
- Confidence: high on MWe; module size and refuelling interval not found (gap)
- Use for the ship: too big for one sector (77 MW would serve 14 sectors), usable only as a drum-scale option; module is built to sit in a water pool, which is heavy on a ship.

### G11. GE Hitachi BWRX-300
- The number(s): 300 MWe net, 870 MWt, 12 to 24 month refuelling cycle (outage 10 to 15 days), reactor pressure vessel 4 m internal diameter, about 26 to 27 m tall (sources disagree; no official height found), 240 fuel bundles.
- Source: GE Vernova SMR technology page (via search summary; the GE product page returned 503) and the UK GDA Step 2 report, https://www.gov.uk/government/publications/gda-step-2-of-the-ge-vernova-hitachi-smr-fundamental-assessment/gda-step-2-of-the-ge-vernova-hitachi-smr-fundamental-assessment-full-report , source's own date: undated in snippet
- Quotation: paraphrase, no exact quotation (SNIPPET)
- Read: 2026-11-12
- Confidence: medium
- Use for the ship: a boiling-water reactor needs gravity (water boils and drains), a 27 m vessel and frequent refuelling; a poor fit for a ship. Reject for the ship, though 1 g spin gravity would make it work in principle.

### G12. Westinghouse eVinci
- The number(s): 5 MWe from a 15 MWth core; designed to run "eight or more full-power years before refueling"; transportable in shipping containers by rail, barge or truck; footprint under 2 acres (8,100 m2).
- Source: Westinghouse, eVinci microreactor page, https://www.westinghousenuclear.com/energy-systems/evinci-microreactor , source's own date: undated
- Quotation (EXACT): "designed to run for eight or more full-power years before refueling"
- Read: 2026-11-12
- Confidence: high that these are the maker's claims; the reactor is not yet operating, so treat as a design claim
- Use for the ship: the natural sector-scale unit: 5 MWe is almost exactly one sector's 5.4 MW.

### G13. Oklo Aurora
- The number(s): 15 to 75 MWe per powerhouse, expandable to 100 MWe and up, runs on fresh, recycled or down-blended fuel; first powerhouse targeted for late 2027 or early 2028. The 20-year fuel life in the brief was NOT confirmed.
- Source: Oklo Inc. Form 10-Q for the period ended 2025-09-30, https://www.sec.gov/Archives/edgar/data/1849056/000162828025051349/oklo-20250930.htm , source's own date: filing for September 30, 2025 (SNIPPET)
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: low-medium
- Use for the ship: not a baseline; no operating unit.

### G14. Project Pele (US DoD)
- The number(s): 1 to 5 MWe (DOE wording); BWXT describes a 1.5 MWe demonstration; high-temperature gas reactor with HALEU TRISO fuel, moved in standard shipping containers; a 20 ton mass was NOT found.
- Source: US DOE Office of Nuclear Energy, "Department of Defense Breaks Ground on Project Pele Microreactor", https://www.energy.gov/ne/articles/department-defense-breaks-ground-project-pele-microreactor (SNIPPET); BWXT delivered the full TRISO core in 2025
- Quotation: paraphrase, no exact quotation
- Read: 2026-11-12
- Confidence: medium
- Use for the ship: confirms gas-cooled TRISO microreactors are reaching test, and TRISO fuel keeps fission products inside each fuel particle (no figure read for the temperature limit).

### G15. NASA Kilopower/KRUSTY and Fission Surface Power
- The number(s): Kilopower: "capable of providing up to 10 kilowatts of electrical power"; KRUSTY test ran 28 hours at full power (Nevada, November 2017 to March 2018, announced May 2, 2018); fission power about 2.75 kW in the test (summary). Fission Surface Power: at least 40 kW, "enough to continuously run 30 households for ten years".
- Source: NASA, "Demonstration Proves Nuclear Fission System Can Provide Space Exploration Power", https://www.nasa.gov/news-release/demonstration-proves-nuclear-fission-system-can-provide-space-exploration-power/ , source's own date: May 2, 2018; and NASA Fission Surface Power page, https://www.nasa.gov/exploration-systems-development-mission-directorate/fission-surface-power/ , undated
- Quotation (EXACT): "capable of providing up to 10 kilowatts of electrical power"; "enough to continuously run 30 households for ten years"
- Read: 2026-11-12
- Confidence: high
- Use for the ship: space reactors are 10 to 40 kW, so roughly 100 times too small per unit; useful only as "the reactor is a machine that was already tested for a ship" evidence. Never claim them as sector-scale.

---

## 3. Fuel per year

### G16. Fission: mass fissioned per unit of heat
- The number(s): about 200 MeV per fission; about 2.7 x 10^21 fissions per megawatt-day; "1 MWd ≈ 1 g fissile nuclides" (U-235: 1.05 g). Calc, 60 MWe net pair at 300 MWt: 300 g/day = 110 kg of fissile material fissioned per year. Fuel loaded is far more (the fuel stays in the core until its burnup). Calc at an assumed 40 GWd per tonne of heavy metal (burnup not sourced for KLT-40S): 109,500 MWd / 40,000 = about 2.7 tonnes of fuel (heavy metal) used per year per pair.
- Source: IAEA, "Nuclear physics and reactor theory, Module I" (Basic Professional Training Course), https://gnssn.iaea.org/main/bptc/BPTC%20Module%20Documents/Module01%20Nuclear%20physics%20and%20reactor%20theory.pdf , source's own date: undated
- Quotation (EXACT from the PDF text): "200 MeV" per fission and "2.7" x 10^21 "fissions" for 1 MWd (the equation was garbled; the "1 MWd ≈ 1 g" sentence is from the search summary, not read in the page text)
- Read: 2026-11-12
- Confidence: high for physics (200 MeV, 1 g/MWd); low for the 40 GWd/t assumption
- Use for the ship: Per Drum 1 (84.2 MWe load), fissile consumption is about 105 kg per year (RITM-200N at 29% efficiency) to 155 kg per year (KLT-40S-like at 20% net), calc.

### G17. Fusion: D-T and D-3He energy per reaction, fuel mass
- The number(s): D-T 17.6 MeV; D-3He about 18.3 MeV (18.354 in a NASA TM; 18.4 elsewhere). ITER: a fusion plant with the output of a 1,000 MW plant needs "250 kgs of fuel per year, half of it deuterium, half of it tritium". Calc, per GW of FUSION power for a year: 1.1 x 10^28 reactions, 56 kg tritium + 37 kg deuterium (about 93 kg); ITER's handbook figures 55.8 kg and a PPPL slide 55.6 kg of tritium per GW-year agree. For 60 MWe at 35% thermal conversion (171 MWt): 9.6 kg tritium + 6.4 kg deuterium = 16 kg per year, at full power all year. D-3He: 18.3 MeV gives, for the same 171 MWt, about 9.3 kg of helium-3 + 6.2 kg of deuterium per year (calc).
- Source: ITER Organization, "Fuelling the Fusion Reaction", https://www.iter.org/sci/fusionfuels , source's own date: undated; 17.6 MeV (arXiv paper, SNIPPET) and 18.354 MeV (NASA/TM-2020-5009040, https://ntrs.nasa.gov/api/citations/20205009040/downloads/NASA-TM-20205009040.pdf , October 2020, SNIPPET)
- Quotation (EXACT): "a fusion plant with the same ouput will only require 250 kgs of fuel per year, half of it deuterium, half of it tritium" (the typo "ouput" is on the page)
- Read: 2026-11-12
- Confidence: high for ITER sentence; high for the physics calc; medium for the MeV values (snippets)
- Use for the ship: fusion would burn about 16 kg of fuel a year per 60 MWe, against about 110 kg fissioned (fission) per year for the same electricity, but see G18: the fuel is the hard part.

### G18. Where the tritium and helium-3 come from
- REMOVED 2026-11-12: congress.gov disallows Claude-User (ITER's tritium-breeding sentence is quoted in the README instead). Its numbers are not used anywhere in this research.

## 4. Siting and the catastrophe case

### G19. Emergency planning zone, large plants and the 2023 small-reactor rule
- The number(s): large plants: plume pathway zone about 10 miles (16 km) radius, ingestion zone about 50 miles (80 km). For small modular reactors and other new technologies electing 10 CFR 50.160, the zone is sized by analysis: the plume EPZ is the area where projected dose "is projected to exceed 10 mSv (1 rem) total effective dose equivalent over 96 hours" and prompt protective measures are needed (as returned by the eCFR tool; the criteria were summarised, not copied in full). The 2020 proposal floated a 2-mile zone; the final rule (published Nov 16, 2023) has no fixed default radius.
- Source: re-read 2026-11-12 on the Legal Information Institute's copy of the regulation, https://www.law.cornell.edu/cfr/text/10/50.47 (the helper first read it through eCFR's API, whose robots.txt disallows Claude-User; the quotations below were confirmed on the LII page).
- Quotation (EXACT, 50.47(c)(2)): "the plume exposure pathway EPZ for nuclear power plants shall consist of an area about 10 miles (16 km) in radius" ; (50.33(g)(2)(i)) "projected to exceed 10 mSv (1 rem) total effective dose equivalent over 96 hours"
- Read: 2026-11-12
- Confidence: high for 10 mi and 1 rem/96 h; medium for the "no default radius" statement (search summary of the final rule)
- Use for the ship: a ship cannot give a 10 mile (16 km) zone: the drum is 2 km long. The rule's logic (draw the line where projected dose reaches 10 mSv in 96 h) is the right test for the ship, applied to the compartment, not a distance.

### G20. Exclusion area and public dose limit
- The number(s): exclusion area: a person at any point on its boundary for 2 hours after a postulated fission product release must not receive more than 25 rem whole body (and 300 rem thyroid). Public dose limit in normal operation: 0.1 rem (1 mSv) a year.
- Source: re-read 2026-11-12 on the Legal Information Institute's copy of the regulation, https://www.law.cornell.edu/cfr/text/10/100.11 and https://www.law.cornell.edu/cfr/text/10/20.1301 (the helper first read it through eCFR's API, whose robots.txt disallows Claude-User; the quotations below were confirmed on the LII page).
- Quotation (EXACT, 100.11): "would not receive a total radiation dose to the whole body in excess of 25 rem"; (20.1301) "does not exceed 0.1 rem (1 mSv) in a year" (the tool gave the second as a core clause)
- Read: 2026-11-12
- Confidence: high
- Use for the ship: two different standards: 1 mSv a year for anyone living near it, 250 mSv in the first 2 hours of an accident at the fence. Homes and gardens aboard should be designed to the 1 mSv/year line in normal operation, and the 25 rem line is the accident case for the nearest door.

### G21. Containment leak rate
- The number(s): the allowed leak is a plant-specific L_a, in percent of containment air per 24 hours at peak accident pressure. Plant technical specifications show 0.2%, 0.3%, 0.5% and 2% per day; test acceptance: periodic Type A test must show less than 0.75 L_a.
- Source: re-read 2026-11-12 on the Legal Information Institute's copy of the regulation, https://www.law.cornell.edu/cfr/text/10/appendix-J_to_part_50 (the helper first read it through eCFR's API, whose robots.txt disallows Claude-User; the quotations below were confirmed on the LII page).
- Quotation (EXACT, Appendix J option A): "The leakage rate Lam shall be less than 0.75 La."
- Read: 2026-11-12
- Confidence: high for the rule; medium for the plant values
- Use for the ship: use a reactor compartment leak rate of 0.1% per day or better (tighter than the best US value), since the compartment is small and the neighbours are 5 m away. This number is my recommendation, not a sourced rule.

### G22. Protective Action Guides
- The number(s): early phase: 1 to 5 rem (10 to 50 mSv) projected over four days; sheltering in place and evacuation are the principal protective actions, with the choice depending on whether evacuation within the time available is feasible and its own risks; potassium iodide when projected child thyroid dose exceeds 5 rem (50 mSv). Late phase: 2 rem in the first year and 500 mrem a year after.
- Source: US EPA, PAG Manual (EPA-400/R-17/001, January 2017), through EPA's webinar slides, https://www.epa.gov/sites/default/files/2017-05/documents/2017_pags_manual_webinar_1may2017.pdf , May 1, 2017
- Quotation (EXACT from the PDF text): "1-5 rem (10-50 mSv) projected over four days."
- Read: 2026-11-12
- Confidence: high
- Use for the ship: write the rule as: if the compartment alarm projects 10 mSv in four days at a home, shelter behind the inner decks; at 50 mSv, move that sector's people to the far end of the drum.

### G23. Gamma shielding: half-value and tenth-value thicknesses
- The number(s): concrete, practical (OSHA): Cs-137 (0.66 MeV): half-value 4.8 cm, tenth-value 15.7 cm; Co-60 (1.17, 1.33 MeV): 6.6 cm and 20.8 cm. Steel: Cs-137 1.6 / 5.3 cm; Co-60 2.1 / 6.9 cm. Water (not in OSHA's table): NIST narrow-beam mass attenuation 0.0896 cm2/g at 0.6 MeV, 0.0787 at 0.8, 0.0632 at 1.25 MeV; calc: Cs-137 half-value about 8 cm, tenth-value about 27 cm; Co-60 half-value about 11 cm, tenth-value about 36 cm (narrow-beam; a thick shield gives more scatter, so use about 1.3x, my estimate).
- Source: OSHA, "Shielding Layer Examples", https://www.osha.gov/ionizing-radiation/introduction/shielding-layer-examples ; NIST, X-Ray Mass Attenuation Coefficients, https://physics.nist.gov/PhysRefData/XrayMassCoef/ComTab/water.html , source's own date: undated
- Quotation (EXACT, table as returned): "Cesium-137 | 0.66 | 4.8 | ... | 15.7"; "Cobalt-60 | 1.17, 1.33 | 6.6 | ... | 20.8"
- Read: 2026-11-12
- Confidence: high for OSHA/NIST values; medium for the water calc
- Use for the ship: each tenth-value layer cuts gamma by 10. For a billion-fold (10^9) cut of Co-60 gamma you need 9 tenth-value layers = 1.9 m of concrete or 3.3 m of water (calc, narrow beam; 4.3 m with a 1.3 safety factor). A 1.5 m water jacket or wall plus a 1 m concrete ring is cheap compared with distance.

### G24. Naval-reactor compartment approach
- The number(s): none found. No primary source read describing the Akademik Lomonosov or a naval reactor compartment (sealed compartment, hull penetrations, leak limits) in numbers. The only Lomonosov material read (WNN) covers output and refuelling.
- Source: World Nuclear News (above); the IAEA conference paper on the plant returned 403
- Quotation: none
- Read: 2026-11-12
- Confidence: n/a
- Use for the ship: gap. If wanted, the IAEA/Rosatom safety descriptions in ARIS are the primary source to read next.

### G25. Filtered venting, HEPA and charcoal
- The number(s): no figure read. Only the general fact (EPA slides): iodine is the thyroid problem, handled by potassium iodide (G22).
- Source: not found in the time available; a targeted search of the NRC Reg. Guide 1.52 (filter efficiency) and 10 CFR 50.34 is the next step
- Quotation: none
- Read: 2026-11-12
- Confidence: n/a
- Use for the ship: gap. Do not state a 99.97% HEPA or 99% charcoal efficiency in the game text until read (the commonly quoted values are from memory, not from this research).

---

## 5. Photovoltaics

### G26. Transparent PV
- The number(s): the theoretical limit for a single-gap visibly fully transparent cell that harvests only ultraviolet and near-infrared light is 20.6%, compared with 33.1% for an opaque black cell (Halme and Makinen 2019, quoting the Lunt group's framework). Demonstrated: "above 5 percent" for highly transparent cells vs about 15 to 18% for ordinary panels (Lunt, 2017). The Traverse, Pandey, Barr, Lunt paper (Nature Energy 2, 849-860, 2017) is a review; its own abstract and numbers could not be opened (paywall/redirect), so its own theoretical-limit sentence is unread.
- Source: Halme and Makinen, "Theoretical efficiency limits of ideal coloured opaque photovoltaics", Energy and Environmental Science 12(4):1274-1285, https://acris.aalto.fi/ws/portalfiles/portal/33410996/SCI_Halme_M_kinen_Theoretical_Efficiency_Accepted_manuscript_with_broader_context_and_TOC_entry.pdf , 1 April 2019 (accepted manuscript); MSU Today news release, https://msutoday.msu.edu/news/2017/10/transparent-solar-technology-represents-wave-of-the-future , October 23, 2017
- Quotation (EXACT, Halme): "the limit for a visibly fully transparent single-band-gap solar cell is 20.6 %, which is ca 2/3 of the 33.1 % theoretical limit" ; (EXACT, MSU) "highly transparent solar applications are recording efficiencies above 5 percent"
- Read: 2026-11-12
- Confidence: high for 20.6/33.1 (peer reviewed); medium for "above 5%" (a university press release, not the paper)
- Use for the ship: a see-through PV window layer has a ceiling of about 20% and a practical value near 5%, and catches only UV and near-infrared. For the ship's lamp-lit gardens this is a rounding error (see G27).

### G27. Printed / emerging PV and the NREL best-cell chart
- The number(s): best research-cell values, row maxima of the NREL (now NLR) cell-efficiency data table: perovskite single junction 28.0% (Hainan U, 2026, 0.05 cm2 cell); organic 19.4% (2025); quantum dot 19.1% (UNIST, 2023); dye-sensitised 13.0% (2020); perovskite/silicon tandem 35.2% (LONGi, 2026); perovskite tandem 30.1% (2024); silicon single crystal (non-concentrator) 26.7% (Trina, 2025); CdTe 23.1%; III-V triple junction (non-concentrator) 39.5%; GaAs thin film 29.1%. All are tiny lab cells (0.05 to 1 cm2). "Solar paint" has no row of its own. Page last updated 2026-08-14 (as read).
- Source: NLR (formerly NREL), Best Research-Cell Efficiency data table, https://www.nlr.gov/docs/libraries/pv/cell-efficiency-data-table.xlsx and https://www.nlr.gov/pv/cell-efficiency , page updated Aug. 14, 2026
- Quotation: none (table values read by script from the .xlsx; I did not copy a sentence)
- Read: 2026-11-12
- Confidence: high for the table; my "row maximum" reading assumed the Combined-efficiency column
- Use for the ship: no listed printed PV exceeds about 19 to 28% in a lab cell. Plan on 10 to 15% for something that can be produced in volume on the ship (my estimate, not sourced).

### G28. Starlight in interstellar space vs sunlight at Earth
- The number(s): total starlight energy density in the solar neighbourhood u = 8.64 x 10^-13 erg per cm3 = 8.64 x 10^-14 J/m3 (calc). Multiply by c for the all-direction flux: about 2.6 x 10^-5 W/m2 (calc), and a flat panel facing one half-sky receives about 6.5 x 10^-6 W/m2 (calc, uc/4). Sunlight at Earth: 1,361 W/m2. Ratio: sunlight at Earth is about 5 x 10^7 times (about 50 million times) stronger than interstellar starlight (calc). So a ship far from any star, with panels the size of its whole 2 km x 250 m hull (about 3 km2), would collect on the order of 0.02 to 0.08 W in total: nothing.
- Source: arXiv astro-ph/9605046, "Radiative Torques on Interstellar Grains: I. Superthermal Spinup", 1996 (author names not read), https://arxiv.org/pdf/astro-ph/9605046 , citing Mezger, Mathis and Panagia 1982 and Mathis, Mezger and Panagia 1983 (A&A 128, 212); separate estimate about 0.43 eV/cm3 in arXiv 1608.01587 (SNIPPET)
- Quotation (EXACT from the PDF text): "The total starlight energy density is uISRF = 8.64 × 10-13 ergs cm-3."
- Read: 2026-11-12
- Confidence: high for the value (standard model); the conversion to W/m2 is my arithmetic. Solar constant 1,361 W/m2 was given by the README; I did not re-read NASA/NOAA.
- Use for the ship: confirmed: no energy from starlight. Only a flyby star (during acceleration or deceleration near a target star) gives meaningful PV. 8.64e-13 erg/cm3 is about 0.54 eV/cm3.

### G30. Heat the ship must reject (calc only, not in the brief)
- The number(s): the 10.8 kW per person eventually all becomes heat; Drum 1 at 84.2 MW of electricity, plus the reactors' own waste heat (about 340 MW at 20% efficiency, about 205 MW at 29%). A radiator at 300 K and emissivity 0.9 rejects about 413 W/m2 per side (calc, sigma T^4); 425 MW total rejection needs about 1.0 km2 of one-sided area, or about 0.5 km2 if two-sided and unshaded (calc). A hotter loop (400 K, 1,306 W/m2 per side) cuts that to 0.16 km2 two-sided. No source read; this is a flag for the design, not a researched number.
- Source: Stefan-Boltzmann law (no page read)
- Quotation: none
- Read: n/a
- Confidence: medium (physics), no source
- Use for the ship: the radiators are probably a bigger structure than the reactors; budget for them, or the drum overheats.

---

## Recommendation (what to use)

Load: 10.8 kW per person (9.4 lamps + 1.4 other). A sector of 500 = 5.4 MWe. Drum 1 (about 7,800 people) = 84.2 MWe. Keep a margin: add 10% for industry and losses, which gives 5.9 MW per sector and 92.6 MW for Drum 1 (calc).

Sector (500 people), if the sector has its own plant: Westinghouse eVinci-class heat-pipe microreactors, 5 MWe / 15 MWt each (G12). Need 2 for 5.9 MW, plus 1 spare: 3 units, 15 MWe installed, any one can fail or be refuelled. Heat 3 x 15 MWt. Fuel: the core is designed for 8 or more years between refuelling. Calc: 5.9 MWe at eVinci's 5/15 = 33% is about 17.7 MWt, and at 1 g per MWd that is about 17.7 g a day, about 6.5 kg a year of fissile material fissioned for the sector.

Drum 1 as one plant (preferred): 3 x RITM-200N-class units, 55 MWe each, 165 MWe installed, any two (110 MWe) cover the 92.6 MWe need (G9). OR 4 KLT-40S reactors (two pairs = 120 MWe net by the project's own figure); any three (90 MWe) nearly cover 92.6, so add a fifth for strict N+1 (G8). Fuel per year fissioned: about 105 kg (RITM-200N at 29%) to 155 kg (KLT-40S at 20% net) for the whole drum, calc (G16); loaded fuel depends on burnup, about 2.7 to 3.9 tonnes of uranium a year at an assumed 40 GWd/t. Fusion: 16 kg per 60 MWe per year but the fuel (tritium, lithium breeding) is not available and helium-3 world supply is about 1 kg a year, so fission is the plan (G17, G18).

Siting rules for homes and gardens (my design rules built from the sources; each cites the number it rests on):
1. Put the reactors outside the pressurised habitat, in vacuum, at the end of the drum or on a boom, behind a shadow shield and a water or concrete wall. A sealed atmosphere shared with the reactor would turn a leak into a plume with no wind to disperse it; the "10 mile" zone (G19) does not scale, so distance and vacuum take its place. Heat goes to the habitat only by sealed liquid loops.
2. Shielding between any reactor and a living space: enough tenth-value layers that normal-operation dose at a home is under 1 mSv a year (G20), for Co-60 class gamma 20.8 cm of concrete per factor of 10 (G23). 9 layers is about 1.9 m of concrete or about 3.3 m of water, so a 2 to 4 m wall.
3. Accident case: treat the reactor compartment boundary like the 10 CFR 100 exclusion boundary: 25 rem in 2 hours (G20) must not be reached at the nearest home. The ship line to hold is the NRC's own: 10 mSv (1 rem) in 96 hours at the edge of the "prompt action" zone (G19).
4. Compartment leak rate: design to 0.1% per day or better (recommendation); test to Appendix J's 0.75 L_a (G21).
5. Protective actions: shelter between decks at 10 to 50 mSv projected over four days; evacuate that sector at 50 mSv and above; potassium iodide stocked for children (G22).
6. Gardens: dose to the public of 1 mSv a year (G20) also covers workers in the gardens; no garden within the water/concrete line of the reactor deck. Lamp banks and radiators, not reactors, are the ship's main heat source (G30).

## Could not read
- https://www.federalregister.gov/documents/2023/11/16/2023-25163/... (redirect to an "unblock" challenge page; I did not follow it). Final rule text read only through summaries.
- https://www.nrc.gov/docs/ML2322/ML23226A218.pdf (403)
- https://www.nature.com/articles/s41560-017-0016-9 (redirected to an authentication page; paywall)
- https://ui.adsabs.harvard.edu/abs/2017NatEn...2..849T/abstract (405)
- https://www-pub.iaea.org/MTCD/Publications/PDF/p15790-PUB9062_web.pdf (402 Payment Required); https://inis.iaea.org/records/3eh40-6pp56 (metadata only); aris.iaea.org PDFs (404) and robots.txt (404)
- https://conferences.iaea.org/event/374/papers/31138/files/12743-Experience%20of%20FNPP%20operation%20(fin).doc (403)
- https://nuclear.gepower.com/... BWRX-300 (503)
- https://www.nrc.gov/reactors/new-reactors/large-lwr/design-cert/nuscale-us600.html (404); NRC EPZ page (403); NRC burnup glossary (403)
- https://doaj.org/article/86813bc9c8b04962bac6ac9909af84f1 and https://www.mdpi.com/1996-1073/13/3/758 (403)
- NASA BVAD 2022 (NTRS 20210024855) and BVAD Rev1 (20180001338.pdf): the PDF content did not come back through the reader; NTRS pages timed out or returned an abstract only
- https://repository.tudelft.nl/... (connection refused; the same abstract was read at research.wur.nl)
- https://www.nrel.gov/ (no DNS; read at nlr.gov instead)

## Negative findings and gaps
- No single published "ISS ECLSS kW per person" or BVAD kW-per-person figure found (G5).
- No DOE horticultural wall-plug efficiency figure found; DOE SSL material found was general (G3).
- DLC V4.0's explicit "2.5" sentence not read; derived from DLC's own +8.7% (G1).
- No source for the 20 year fuel life of Oklo Aurora, no 20-ton mass for Project Pele, no module dimensions for NuScale, no official vessel height for BWRX-300, no vessel dimensions for KLT-40S (G10, G11, G13, G14).
- Filtered venting, HEPA and charcoal efficiency: not researched to a source (G25). Naval reactor compartment primary description: not found (G24).
- Helium-3: the CRS and GAO data are 2011 to 2015; no current DOE sale price confirmed (G18).
- Tritium start-up inventory for a fusion plant (kg needed to start, and who has it): not researched. World civil tritium stock is in the tens of kilograms by common statements, but I read no source, so treat as unknown.
- The ITER sentence refers to a plant "with the same output" as a 1000 MW coal plant; the ITER page does not say whether MW is electric or thermal. My calc shows 250 kg/yr fits a 1 GWe plant.
- Burnup of KLT-40S and RITM-200N fuel: not found; the 40 GWd/t figure used in G16 is my assumption.
- Waste-heat rejection (G30) is outside the brief and has no source read.

## Summary table
| Question | Recommended number | Findings | Confidence |
|---|---|---|---|
| LED efficacy | 2.8 umol/J (floor 2.3 now, 2.5 from 2025; ceiling about 3.4 to 4.1) | G1, G2 | high (floor), medium (ceiling) |
| LED wall-plug | about 60% (calc) | G3 | medium |
| Vertical-farm energy | 247 kWh per kg dry lettuce, about 6 kW per person for food, so 9.4 kW lamps is generous | G4 | high (abstract), assumption on diet |
| Life support per person | about 0.8 kW before cooling | G5 | low-medium |
| Electricity per person (2023) | US 12,711 kWh (1.45 kW), world 3,578 kWh (0.41 kW) | G6 | high |
| Steel | 0.4 to 0.5 MWh/t scrap EAF; about 3.7 MWh/t H2 route | G7 | medium-low |
| Reactor sizes | KLT-40S 35 MWe; RITM-200N 55 MWe (190 MWt, 8.7 x 21 m, 1,500 t); NuScale 77 MWe; BWRX-300 300 MWe; eVinci 5 MWe, 8+ years; Kilopower 10 kWe; FSP 40 kWe | G8 to G15 | medium to high |
| Fission fuel | 1 g U-235 per MWd thermal: about 110 kg per 60 MWe-pair-year; Drum 1 about 105 to 155 kg a year | G16 | high (physics) |
| Fusion fuel | 16 kg/yr per 60 MWe (D-T); helium-3 supply about 1 kg/yr worldwide | G17, G18 | high (calc), medium (supply) |
| EPZ | 10 mi large plants; SMR zone = 10 mSv in 96 h line | G19 | high |
| Exclusion and public dose | 25 rem in 2 h at boundary; 1 mSv/yr | G20 | high |
| Containment leak | plant-specific 0.2 to 2% a day; test less than 0.75 L_a; ship target 0.1% | G21 | high (rule), recommendation |
| PAG | 1 to 5 rem over 4 days | G22 | high |
| Shielding | concrete TVL 15.7 cm (Cs-137), 20.8 cm (Co-60); water about 27 / 36 cm (calc) | G23 | high / medium |
| Transparent PV | limit 20.6%, demonstrated above 5% | G26 | high / medium |
| Printed PV | best lab: perovskite 28.0%, organic 19.4%, quantum dot 19.1% | G27 | high |
| Starlight | 8.64e-13 erg/cm3; about 5e7 times fainter than sunlight at Earth | G28 | high |
| Reactor model and count | Sector: 3 x eVinci-class (5 MWe). Drum 1: 3 x RITM-200N-class (or 4 to 5 KLT-40S) | all | medium |
