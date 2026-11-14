# Slag as building material: research notes (sl-B)

Researched 2026-11-13. Every number below was read from the named source in this session; robots.txt was
fetched for each site before its pages. Quotations are copied as the fetch tool returned them (PDF text
extracted with pdftotext loses subscripts: "SiO" in an extracted quote means SiO2, "AlO" means Al2O3).

---

## 1. Ground granulated slag (slag cement, GGBS) and alkali-activated slag

**B1. Quench to glass vs air-cool to rock (the fork that decides cement or aggregate)**
- Number: none; the process definitions.
- Source: FHWA, *User Guidelines for Waste and Byproduct Materials in Pavement Construction*, FHWA-RD-97-148, chapter "Blast Furnace Slag, Material Description". https://www.fhwa.dot.gov/publications/research/infrastructure/structures/97148/bfs1.cfm
- Source date: the report number dates it 1997 (archived page, last modified 2016-03-08; the page states no date of its own).
- Quote: "If the molten slag is cooled and solidified by rapid water quenching to a glassy state, little or no crystallization occurs." Air-cooled: "If the liquid slag is poured into beds and slowly cooled under ambient conditions, a crystalline structure is formed," (fragment).
- Confidence: high.
- Use for the ship: the smelter's one choice per pour: water-quench (glass, becomes cement) or slow-cool in beds (crystalline rock, becomes aggregate or cast stone). Expanded/foamed and pelletized slag are two more quench routes in the same chapter.

**B2. Grinding energy for granulated slag, measured (vertical roller mill)**
- Numbers: 28.54 kWh/t total at 3290 Blaine (mill 28.36 + classifier 0.18); corrected to 3500 Blaine 30.30 kWh/t (supplier guarantee 24.7 kWh/t); whole grinding shop estimated 40.0 kWh/t.
- Source: CORDIS, EU project IN.-00026-94 (Lafarge pilot of a LOESCHE vertical mill for slag), https://cordis.europa.eu/project/id/IN.-00026-94
- Source date: project 1994-08-01 to 1996-01-01; tests March-April 1995.
- Quotes: "absorbed power : 28,36 kWh/t mill motor"; "so 28,54 kWh/t in total"; "average fineness : 3290 BLAINE (instead of 3500 BLAINE)". Ball mills: "high BLAINE slag manufacturing is with ball mills technically doubtfull," (electricity "dramatic").
- Confidence: high for the measured mill figure; the 40 kWh/t shop figure is the project's own estimate, not measured.
- Use for the ship: grinding glassy slag to cement fineness costs about 30 kWh/t at the mill, about 40 kWh/t for the whole shop (fans, conveying, classifier). Finer grinding (4000-5500 cm2/g, see B5) costs more.
- Disagreement: a web-search snippet attributes "40-50 kwh/t" at 400-500 m2/kg to a US patent (USPTO 12378158); not read directly (USPTO PDF host not opened), so not used.

**B3. Energy and water to granulate one tonne of slag (EPD, cradle to gate)**
- Numbers: total non-renewable primary energy (PENRT) A1-A3 = 446 MJ per tonne GBFS (A3 manufacturing alone 248 MJ); renewable primary energy 9.36 MJ; net fresh water (FW) 0.202 m3 per tonne. GWP A1-A3 28.749 kg CO2e per tonne, of which granulation electricity 14.382 kg.
- Source: Kiwa-Ecobility Experts, *Environmental Product Declaration: GBFS - Granulated Blast-Furnace Slag*, ReTHiNK-91088 (HKM, Duisburg), https://hkm.de/wp-content/uploads/2025/12/EPD-Kiwa-EE-191088-EN.pdf
- Source date: issue date 21-08-2025, valid until 21-08-2030.
- Quote: energy and water are table only, no sentence. Sentence: "This is largely due to the electricity consumption for granulation, which alone accounts for 14.382 kg CO-equivalents." Process: "the slag is fed into a granulating plant where it is quenched with water and converted into a fine-grained, glassy, sand-like product".
- Confidence: high (verified EPD). Note PENRT is primary energy (fuel at the power station), not electricity at the plug; A1 includes allocated upstream.
- Use for the ship: granulation itself is cheap (a few hundred MJ primary per tonne, mostly pumping quench water) and consumes about 0.2 m3 of fresh water per tonne net (most quench water is recirculated; that figure is net loss to steam). On the ship the quench water is a closed loop with heat recovery.

**B4. Chemical acceptance rule for granulated slag in cement (EN 197-1, as quoted in the EPD)**
- Numbers: CaO + SiO2 + MgO > 66%; (CaO + MgO)/SiO2 > 1; glass content > 66%. Typical HKM slag: CaO 42, SiO2 34, MgO 7.2, Al2O3 10, Fe total 0.6 (%).
- Source: the same EPD as B3, section 2.3.
- Source date: 21-08-2025.
- Quote: "In order for granulated blast furnace slag (GBFS) to be used as an additive in cement in accordance with EN 197-1, it must meet the following chemical criteria:" then "CaO + SiO + MgO > 66%", "(CaO + MgO) / SiO > 1", "Glass content > 66 %".
- Confidence: high that the EPD says this; the standard itself was not read (paywalled).
- Use for the ship: the basicity rule counts MgO with CaO, so a magnesium-rich, lime-poor asteroid slag can still pass (CaO+MgO)/SiO2 > 1 if silica is not dominant. A silica-rich stony-asteroid slag will fail it unless lime or magnesia is added as flux. This is the gate the game should check per pour.
- Related (not verified): a producer certificate snippet listed MgO "8 max" vs "18" for EN 15167-1; not opened, not used.

**B5. Alkali activation: activator dose, water, fineness (no Portland cement at all)**
- Numbers: optimal Na2O/binder 5.5% to 8% for slag-based alkali-activated material; SiO2/Na2O (silicate modulus) 0.85 to 1.4; water/binder about 0.4; optimal slag fineness 4000-5500 cm2/g Blaine; British Standard minimum glass content 90%.
- Source: Sun B., Ye G., de Schutter G., "A review: Reaction mechanism and strength of slag and fly ash-based alkali-activated materials", *Construction and Building Materials* 326 (2022) 126843. https://repository.tudelft.nl/file/File_ce6118ee-0719-49b2-bbf5-75744280fbcd (green open access copy)
- Source date: 2022.
- Quotes: "The optimal values of the Na2O/binder ratio of BFS-AAM and FA-AAM are between 5.5% and 8% and between 7 and 10%, respectively." "A w/binder ratio of around 0.4 may strike a balance between strength and workability." "the optimal fineness of BFS that is suitable for AAM is in the range of the Blain value of 4000-5500 cm2/g" (dash normalised). "The British Standard requires a minimum glass phase content of 90% [41]."
- Confidence: high.
- Use for the ship: per tonne of slag binder, about 55-80 kg Na2O-equivalent of activator (sodium hydroxide plus sodium silicate) and about 400 kg water. Sodium is the consumable the ship must supply or recycle; the slag itself needs no lime or clinker on this route.

**B6. Magnesium-rich slag is GOOD for alkali activation**
- Numbers: slags with MgO > 5% form hydrotalcite as the main secondary product; higher MgO = higher extent of reaction and better carbonation resistance (tested up to 7.5 wt.% MgO).
- Source: Bernal S.A., San Nicolas R., Myers R.J., Mejía de Gutiérrez R., Puertas F., van Deventer J.S.J., Provis J.L., "MgO content of slag controls phase evolution and structural changes induced by accelerated carbonation in alkali-activated binders", *Cement and Concrete Research* 57 (2014) 33-43, doi 10.1016/j.cemconres.2013.12.003; record at https://eprints.whiterose.ac.uk/97155 (abstract only; full text not on that page).
- Source date: March 2014.
- Quotes (abstract): "AAS with lower MgO content reacts faster at early age, forming gismondine and C-A-S-H type gels"; "hydrotalcite appears to act as an internal CO2 sorbent". Supporting, Sun et al. 2022 (B5): "The increase of MgO content could lead to faster reaction and higher strength of AAM because a higher amount of hydrotalcite is formed".
- Confidence: high for the 0-7.5% MgO range studied; asteroid slag may run far higher (tens of %) where no study was read: open question.
- Use for the ship: a Mg-rich asteroid slag suits the alkali-activated route better than the Portland-blend route; beyond about 7.5% MgO the behaviour is untested in what I read.

**B7. Replacement share of Portland cement (the bound on the blend route)**
- Numbers: normal range 20 to 80% of cementitious material by mass; paving and structural 25 to 50%; durability specs up to 70%; mass concrete 50 to 80%. Slag cement "nearly 90% less energy" per ton than Portland cement.
- Source: Slag Cement Association, "FAQs", https://www.slagcement.org/faqs
- Source date: no page date; footer "Copyright 2026".
- Quotes: "normally ranges from 20 to 80 percent"; "typically use between 25 and 50% slag cement"; "Mass applications use between 50 and 80 percent slag cement"; "a ton of slag cement requires nearly 90% less energy to produce than a ton of portland cement". Process: "Molten slag is tapped from a blast furnace, rapidly quenched with water ("granulated"), dried and ground to a fine powder."
- Confidence: high that SCA says it; the 90% claim is an industry-association figure (consistent with B2/B3: ~30-40 kWh/t grinding plus drying, versus clinker burning).
- Use for the ship: on the blend route slag replaces at most about 80% of the binder, so the ship still needs 20%+ Portland cement (which needs limestone, absent on asteroids). The alkali route (B5) avoids this bound and is the ship-sensible one. The SCA PDF info sheets (filesusr.com host) refused robots.txt (403), so only the HTML FAQ was read.

---

## 2. Slag aggregate, road base, pavers and bricks

**B8. Air-cooled blast-furnace slag as aggregate: density, absorption, abrasion**
- Numbers: specific gravity 2.0-2.5 (Table 3-1, table only); compacted unit weight up to 1940 kg/m3 for recent ACBFS; expanded slag 800-1040 kg/m3; pelletized 840 kg/m3; water absorption up to 6%; LA abrasion 35-45% (Table 3-3, table only). 1985 NSA mean chemistry CaO 39, SiO2 36, Al2O3 10, MgO 12% (Table 3-2, table only). Slag is "about 20 percent by mass of iron production".
- Source: FHWA-RD-97-148, Blast Furnace Slag Material Description (as B1). https://www.fhwa.dot.gov/publications/research/infrastructure/structures/97148/bfs1.cfm
- Source date: 1997 (report number); page archived.
- Quotes: "some recently produced ACBFS was reported to have a compacted unit weight as high as 1940 kg/m3 (120 lb/ft3)." "The water absorption of ACBFS can be as high as 6 percent." "Pelletized blast furnace slag has a unit weight of about 840 kg/m3 (52 lb/ft3)."
- Confidence: high.
- Use for the ship: crushed air-cooled slag is a light, porous aggregate (bulk about 1.1-1.9 t/m3); processing energy is only crushing and screening (no figure found; open question). Foamed or pelletized slag (0.8-1.0 t/m3) is a lightweight filler for floors or shielding bulk.

**B9. Steel slag expands: the free-lime and free-magnesia caveat**
- Numbers: volume change up to 10% or more from hydration of free CaO and MgO; up to 18 months of outdoor ageing to hydrate them; specific gravity 3.2-3.6; unit weight 1600-1920 kg/m3; LA abrasion 20-25% (tables).
- Source: FHWA-RD-97-148, Steel Slag, Material Description. https://www.fhwa.dot.gov/publications/research/infrastructure/structures/97148/ssa1.cfm
- Source date: 1997 (report number).
- Quotes: "Steel slag aggregates generally exhibit a propensity to expand."; "volume changes of up to 10 percent or more attributable to the hydration of calcium and magnesium oxides"; "Up to 18 months may be needed to hydrate the expansive oxides."
- Confidence: high.
- Use for the ship: any slag carrying UNREACTED (free) MgO or CaO crystals swells when it meets water. A magnesium-rich asteroid slag that is air-cooled may hold free periclase (MgO); it must be quenched fully to glass, or aged/steamed, or tested before it goes into anything confined (a wall, a floor). This is the main failure mode to model.

**B10. Carbonated slag compacts (bricks and pavers cured with CO2)**
- Numbers: CO2 at up to 2 MPa and 20-140 °C, up to 16 h; compressive strength above 100 MPa possible; 100-150 g CO2 sequestered per kg slag; compaction pressure 75-609 kgf/cm2 (about 7-60 MPa).
- Source: Quaghebeur M., Nielsen P., Horckmans L., Van Mechelen D., "Accelerated Carbonation of Steel Slag Compacts: Development of High-Strength Construction Materials", *Frontiers in Energy Research* 3:52. https://www.frontiersin.org/articles/10.3389/fenrg.2015.00052/full
- Source date: 17 December 2015.
- Quotes: "A compressive strength of more than 100 MPa can be obtained."; "the carbonated materials sequester 100–150 g CO2/kg slag."; "treating different types of steel slags with CO2 at elevated pressure (up to 2 MPa) and temperatures (20–140°C)."
- Confidence: high for stainless and BOF steel slags (lime-rich). An Mg-silicate asteroid slag glass will carbonate more slowly; not tested in this source.
- Use for the ship: a CO2 sink as well as a brick: life-support CO2 pressed into slag blocks at 2 MPa makes pavers and blocks with no cement. Slag per tonne of product about 0.87-0.9 t (the rest is bound CO2 and water).
- Could not read: MDPI review "A Review on the Carbonation of Steel Slag" (Materials 17(9) 2066, 2024): mdpi.com refused robots.txt (403).

**B11. Fired clay-slag bricks and unfired GGBS bricks (secondary)**
- Not verified from a primary page in this pass: a web-search summary reported fired bricks with 10-20 wt% slag at 1000-1100 °C and unfired GGBS bricks of 13.18-25.48 MPa. No page read; not to be used as figures. Open question.

**B12. Energy to crush and screen aggregate (applies to air-cooled slag)**
- Numbers: crushing stages at two US quarries 1.77 kWh/t total (jaw 0.35, cone 0.26, cone 1.17), 1.57 kWh/t after mine-to-mill optimisation; whole quarry chain (overburden, blasting, haulage, crushing, screening) 9.7 kWh/t crushed rock, 8.3 kWh/t sand and gravel (Tarmac 2008 data).
- Source: Mitchell C. (British Geological Survey), "Aggregate Carbon Demand: The hunt for low-carbon aggregate", slides, https://nora.nerc.ac.uk/id/eprint/11458/1/EIG_2010_Aggregate_Carbon_Demand_Clive_Mitchell_BGS.pdf
- Source date: 2010 (EIG conference; data for 2008 calendar year).
- Quote: table only, no sentence ("Total, kWh/t 1.77 1.57"; "Crushed rock 4.32 9.7").
- Confidence: medium-high (conference slides citing company reports).
- Use for the ship: crushing air-cooled slag costs about 2 kWh/t (no blasting or haulage aboard), negligible next to melting. Aggregate is the cheapest per-tonne sink.

---

## 3. Cast basalt, cast slag, slag glass-ceramics (petrurgy)

**B13. Cast regolith/basalt for a lunar base: temperature, energy per ton, properties (NASA-funded study)**
- Numbers: liquefy at 1200-1300 °C, cast, controlled cooling; furnace "ENERGY: 1.3 x 10 9 j per ton" (1.3 GJ/t = about 360 kWh/t); 100 kW furnace, 90% efficient, 24 h per 6-ton load (my arithmetic: 2400 kWh / 6 t = 400 kWh/t at the plug, consistent); melt viscosity 100-1000 poise. Cast basalt properties (figure only): ultimate tensile 36 MPa, compressive 550 MPa, Young's modulus 110 GPa, density 2.9 g/cm3 (vs concrete 6 / 76 MPa and 2.4 g/cm3 in the same figure).
- Source: Rogers W.P., Sture S., "Indigenous lunar construction materials", in *Space Construction Activities* (NASA/USRA collected works), NTRS 19930019931. https://ntrs.nasa.gov/api/citations/19930019931/downloads/19930019931.pdf
- Source date: 1 November 1991 (NTRS record).
- Quotes: "It involves liquification of the regolith at 1200-1300°C, casting the liquid into a form, and controlled cooling." "With a cast basalt building material which is brittle, there is the additional problem of connecting the joint with the material and avoiding stress concentration that would cause failure." Energy and properties: figure only (Fig 12.1, 12.2, 12.5).
- Confidence: medium (a 1991 design study; the property table describes terrestrial cast basalt; the energy is a design estimate).
- Use for the ship: casting slag into tiles, blocks or pipe linings costs about 0.36-0.4 MWh per tonne if melted from cold; if poured straight from the smelter while still molten, only holding heat and the slow annealing cool remain. Strong in compression (550 MPa), weak in tension (36 MPa), brittle: good for floors, linings and armour tiles loaded in compression; joints are the design problem.

**B14. Fused cast basalt in industry today (Kalenborn ABRESIST)**
- Numbers: operating temperature max 350 °C; pipes 40-610 mm internal diameter; ash pipes lined with it last more than 30 years (one US plant runs over 10,000 m of 294 mm pipe).
- Source: Kalenborn, "Wear protection for plant components and pipe systems" (product brochure), https://www.kalenborn.com/wp-content/uploads/2023/06/EN_Productbrochure_Wear_Protection.pdf ; product page https://www.kalenborn.com/products/fused-cast-basalt-abresist/
- Source date: brochure uploaded June 2023; product page undated (footer 2026).
- Quotes: "selected natural basalt is melted and cast into molds, then moved into an annealing furnace." "The material is corrosion resistant to most acids and alkalis." "Worldwide ash removal pipes lined with ABRESIST reach lifetimes of more than 30 years." Uses: "in bunkers, gutters, chutes, hoppers, mixers, classifiers and cyclones as well as pipes and bends."
- Confidence: high for what the maker states; no numeric density or strength on the pages read.
- Use for the ship: cast slag as wear linings for ore chutes, hoppers, slurry pipes and the drones' ore-handling gear; a real industrial product line, made by this firm since the 1920s ("by the early 1920's our company successfully produced a basalt based abrasion resistant material").

**B15. Slag glass-ceramics ("slag sitall"): only weak sources found**
- Numbers: none verified.
- Sources: (a) 911Metallurgist blog post "Slags as Ceramic Raw Materials", D. Michaud, 29 Dec 2017, https://www.911metallurgist.com/slags-ceramic-raw-materials : "The Soviets have been using slag glass-ceramics in the construction industry for some time." "Large slabs, formed continuously by a float glass process and heat treated in a single step" (fragment). The post's own lab data: crystallisation at 880-920 °C and above 990 °C (its tests, not Soviet work). (b) British Glass knowledge-base record of Frantsenyuk L.I. et al., "Synthesis of slag-glass ceramics (review)", *Glass and Ceramics* 53(11-12) 356-361 (Novolipetsk Steel), https://www.britglass.org.uk/knowledge-base/digital-library-and-information-services/synthesis-slag-glass-ceramics-review (record only, no figures).
- Source date: (a) 2017 (secondary; original report unnamed); (b) record undated (volume 53 of Glass and Ceramics; year not on the page).
- Confidence: low for any specifics.
- Use for the ship: the glass-ceramic route exists (melt, form, then a nucleation and crystallisation heat treatment near 900 °C) and gives floor tiles and cladding slabs; numbers need a better source (Pavlushkin's Soviet texts, or Frantsenyuk in full).
- Could not read: PMC8123903 (sintered glass-ceramics, self-glazed materials and foams from metallurgical waste slag, Materials 2021): pmc.ncbi.nlm.nih.gov showed a reCAPTCHA browser check; europepmc.org disallows ClaudeBot; mdpi.com refused robots.txt.

---

## 4. Basalt fibre and slag (mineral) wool

**B16. Continuous basalt (rock) fibre: temperature, composition window, strength threshold**
- Numbers: industry melts rock at about 1500 °C, then draws fibre at 1380-1440 °C (viscosity window "10-30 Pa" as printed, cited to Johannesson et al. 2019); "high-performance" fibre per GB/T 38111: modulus over 90 GPa, tensile over 2500 MPa; suitable basalt composition (table, Johannesson et al. 2019 as cited): acidity modulus (SiO2+Al2O3)/(CaO+MgO) 3-8.5, MgO 3-7%, CaO 6-12%, Al2O3 12-19%; adding 5-15 mol% MgO raised tensile strength up to 43% (Kuzmin et al. 2017 as cited).
- Source: Ma Y., Guo Z., Fu J., Xi X., Ma P., Wang X., "From Rock to Fiber: The Mechanical Properties of Continuous Rock Fibers", *Natural Resources Research* (Springer), doi 10.1007/s11053-025-10483-0; open copy https://ira.lib.polyu.edu.hk/bitstream/10397/114855/1/s11053-025-10483-0.pdf
- Source date: published online 15 April 2025.
- Quotes: "In the industry, rocks are first melted at approximately 1500 °C and held for homogenization." "A ``high-performance CRF,'' according to the standard GBT/T38111, should have an elastic modulus of over 90 GPa and tensile strength of over 2500 MPa." "Kuzmin et al. (2017) added 5 to 15 mol % MgO to the bare CRF, which led to a constant increase of tensile strength up to 43%." Composition window: table only.
- Confidence: high (2025 review).
- Use for the ship: continuous fibre needs a silica-rich melt (acidity modulus 3 or more); a typical slag (modulus about 1) cannot be drawn into continuous filament without adding a lot of silica, so continuous fibre is NOT a large slag sink. Some MgO is fine and even helps.

**B17. Continuous basalt fibre energy per kg (trade source only)**
- Number: all-electric melting "comprehensive unit power consumption is 3.0-3.5 kWh/kg"; flame furnaces' heat utilisation "less than 30%".
- Source: China Beihai Fiberglass Co. (basaltbest.com), "Two mainstream continuous basalt fiber preparation technologies", https://basaltbest.com/news/two-mainstream-continuous-basalt-fiber-preparation-technologies/
- Source date: 2025-05-29.
- Quote: "the comprehensive unit power consumption is 3.0-3.5 kWh/kg"
- Confidence: medium-low (manufacturer's trade article). Disagreeing figures seen only in search snippets, NOT read: 3-4 kWh/kg in a review (PMC9227501; PMC showed a robot check), about 5 kWh/kg in a BFRP paper (core.ac.uk 249982377; 403), 1.1 kWh/kg melt-only in a small electromagnetic reactor and 3-7 kWh/kg for slag mineral-wool furnaces in a Russian paper (Springer; robots disallow ClaudeBot). Listed as disagreement, not as figures.
- Use for the ship: about 3-3.5 MWh per tonne of continuous fibre, roughly ten times cast basalt; worth it only for reinforcement (rebar, cloth), not as a bulk sink.

**B18. Slag wool from molten blast-furnace slag: composition limit, temperature, energy, strength**
- **Not cited (note added 2026-11-13 by the design pass):** mdpi.com refused its robots.txt, and
  this entry was read from a re-hosted copy on another site. Out of caution the design and the
  dated finding do not rest on it; read the paper at its publisher, or by hand, before using a
  figure from it.
- Numbers: fiberising needs melt viscosity 0.5-2.5 Pa·s and a melt 50 °C above liquidus; pure BF slag (acidity coefficient Mk 1.01; CaO 39.93, SiO2 32.52, Al2O3 15.95, MgO 8.86) gave only "slag shots" (beads); fibre formed with 20-80% BF slag, the rest coal ash (table: Mk 1.20 at 90% slag up to 3.32 at 40%); qualified fibre under 7 µm; injection 1500-1600 °C (1550 °C chosen), air 0.65-0.85 MPa; slag-fibre tensile strength 2053-2887 MPa; long-term use limit 850 °C, Tg 894-901 °C. Energy: electric furnace fed MOLTEN slag 1170.80 kWh per ton of slag wool vs traditional cupola 0.49 t standard coal per ton (incl. 325.20 kWh electricity); 69.39% reduction. Cupola coal 350-800 kg standard coal per ton depending on technology.
- Source: Zhao D., Zhang Z., Tang X., Liu L., Wang X., "Preparation of Slag Wool by Integrated Waste-Heat Recovery and Resource Recycling of Molten Blast Furnace Slags: From Fundamental to Industrial Application", *Energies* 7 (2014) 3121-3135, doi 10.3390/en7053121. Read from a re-hosted copy, https://inspectapedia.com/insulation/Slag-Wool-Preparation-Zhao-ENergies.pdf (mdpi.com itself refused robots.txt with 403; the article is open access).
- Source date: published 9 May 2014.
- Quotes: "The slag viscosity of 0.5-2.5 Pa·s and the slag temperature of 50 °C higher than the liquidus temperature are generally thought to be suitable parameters for the slag wool production" (dash normalised). "It is clear that the slag fibers can be fabricated when the BF slag content is in the range of 20%-80%, while only slag shots were produced when the slag content is above 80%." "1170.80 kWh (equivalent to 0.15 tons standard coal) is consumed to produce one ton of slag wool while 0.49 tons standard coal/ton is the average consumption for the traditional cupola method." "the maximum long term use temperature of slag wool products should not exceed 850 °C."
- Confidence: high (peer-reviewed, with an industrial plant's measured data).
- Use for the ship: slag wool takes 0.5-0.8 t slag per tonne of wool and needs a silica/alumina additive to lift the acidity coefficient to about 1.2-1.7; from MOLTEN slag about 1.2 MWh/t (electric); from cold feed about 3.3x more (0.49/0.15 by the paper's own coal equivalence, my arithmetic: about 3.8 MWh/t). A magnesium-rich, silica-poor asteroid slag will make beads, not fibre, unless silica is added.

**B19. Slag wool in the North American industry (NAIMA)**
- Numbers: slag wool uses about 70% blast-furnace slag (rest natural rock); rock wool at least 70-75% natural rock (rest slag); over 90% of insulation slag bought direct from steelmakers; over 13 billion pounds of slag used 1992-2005; resists over 2,000 °F.
- Source: NAIMA (North American Insulation Manufacturers Association, now the Insulation Institute), "Rock and Slag Wool Insulation: Sustainable Choices for Conserving Energy", pub. no. N046, https://insulationinstitute.org/wp-content/uploads/2016/01/N046-Rock-and-Slag-Wool-Insulation-Sustainable-Choices-for-Conserving-Energy.pdf
- Source date: "PUB. NO. N046 9/06" (September 2006).
- Quotes: "Typically, slag wool insulation uses approximately 70 percent blast furnace slag, with the remaining volume of raw materials being natural rock." "Manufacturers use a mechanized process to spin a molten composition of rock and slag into high temperature-resistant fibers." "The insulation can resist temperatures in excess of 2,000°F".
- Confidence: high (trade association, dated).
- Use for the ship: confirms the commercial 70% slag share (agrees with Zhao's 50-80% window).

**B20. Mineral wool thermal resistance**
- Numbers: R-3 to R-4 per inch (ft2·h·°F/Btu·in); rigid boards about R-4 per inch. My conversion (k = 0.1442 / R-per-inch, W/m·K): about 0.036-0.048 W/m·K.
- Source: Aldrich R., Mantha P., Puttagunta S. (CARB), *Measure Guideline: Basement Insulation Basics*, US DOE Building America, https://www1.eere.energy.gov/buildings/publications/pdfs/building_america/measure_guide_basement_insul.pdf
- Source date: October 2012.
- Quotes: "R-value: 3-4 per inch (ft2h°F/Btu-in)"; "Mineral wool is manufactured from basalt or other minerals often in combination with slag, a byproduct from several industrial processes."
- Confidence: high.
- Use for the ship: slag wool insulates about as well as fibreglass (0.04 W/m·K class) and does not burn; bounded by the area of walls and pipes to insulate.

---

## 5. Glass from slag or regolith

**B21. What clear glass is made of (the composition slag is far from)**
- Numbers (mass %): SiO2 72.26, Na2O 13.78, CaO 10.05, MgO 3.40, Al2O3 0.10, Fe2O3 0.021, TiO2 0.033, SO3 0.27, K2O 0.021 (round-robin analysis by 8 laboratories).
- Source: Deutsche Glastechnische Gesellschaft (DGG), "Standard Glass 2 of the DGG: Soda-Lime-Silica Glass", https://www.hvg-dgg.de/fileadmin/Standard_Glass_2_DGG.pdf (site serves no robots.txt: 404, so nothing is disallowed).
- Source date: PDF created 2022-02-03; prices quoted "(Jan. 2022)".
- Quotes: "Standard Glass 2 of the DGG is a soda-lime-silica glass (close to float glass)." Composition: table only.
- Confidence: high.
- Use for the ship: clear glass is about 72% silica, 14% soda, 10% lime, and iron held to about 0.02%. Slag (about 35% silica, 7-12% or more MgO, iron-bearing) cannot become clear glass without adding most of the silica and all the soda; slag's glass is dark, coloured or opaque. Clear windows need a separate silica-purification line (the regolith is the feed, not the slag).

**B22. Foamed glass-ceramic insulation from slag (needs waste glass)**
- Numbers: batch 30 slag : 70 waste glass : 10 additives; borax flux 6-10%, best at 8%; 400 °C hold 20 min, then 10 °C/min to 900 °C, 30 min; product bulk density 0.82 g/cm3, compressive strength about 25 MPa. The slag was water-quenched high-titanium BF slag (SiO2 24.72, CaO 28.39, Al2O3 13.64, TiO2 21.40%).
- Source: Sun Q., Wang H. (Panzhihua University), "Effects of Na2B4O7·5H2O on Foam Glass-Ceramics Prepared from High Titanium Blast Furnace Slag", 4th International Conference on Material Engineering and Application (ICMEA 2017), *Advances in Engineering Research* vol. 146, Atlantis Press, https://www.atlantis-press.com/article/25889527.pdf
- Source date: conference 2017; "Copyright © 2018, the Authors".
- Quotes: "its compressive strength was about 25MPa, the bulk density was 0.82g / cm3." "the ratio between the three was 30: 70: 10." "heated at a heating rate of 10 °C / min to 900 °C, after 30 min heat preservation, the foam glass was prepared."
- Confidence: medium (short conference paper, one slag type; no thermal conductivity reported).
- Use for the ship: a strong, light (0.8 t/m3) foamed tile is possible at a modest 900 °C, but in this recipe slag is only 30% of the batch; the rest is glass. As a slag sink it is small unless the ship has a glass waste stream. Search snippets also mention slag foams with conductivity 0.12-0.43 W/m·K (not read).

---

## 6. Sintered and 3D-printed blocks from regolith

**B23. ESA solar-sintered bricks, 2017 (DLR solar furnace, Cologne)**
- Numbers: 0.1 mm layers baked at 1000 °C; a 20 x 10 x 3 cm brick in about five hours; strength "equivalent" to gypsum; edges warp because they cool faster. (A later review gives the RegoLight solar-sintered material's compressive strength as 2.49 MPa, Young's modulus 0.21 GPa: see B26.)
- Source: ESA, "Printing bricks from moondust using the Sun's heat", https://www.esa.int/Enabling_Support/Space_Engineering_Technology/Printing_bricks_from_moondust_using_the_Sun_s_heat
- Source date: 03 May 2017.
- Quotes: "This was done on a 3D printer table, to bake successive 0.1 mm layers of moondust at 1000°C." "We can complete a 20 x 10 x 3 cm brick for building in around five hours." "The resulting bricks have the equivalent strength of gypsum, and are set to undergo detailed mechanical testing."
- Confidence: high (that ESA says it); strength is a comparison, not a measurement.
- Use for the ship: solar or lamp sintering is slow (600 cm3 of brick in about 5 h on this rig) and weak; good for paths and non-structural blocks, not hulls.

**B24. ESA / Monolite D-Shape printed block, 2013 (binder route, not heat)**
- Numbers: 1.5 tonne demonstration block; printer builds about 2 m per hour; regolith simulant mixed with magnesium oxide, then a binding salt sprayed on; a hollow closed-cell structure.
- Source: ESA, "Building a lunar base with 3D printing", https://www.esa.int/Enabling_Support/Space_Engineering_Technology/Building_a_lunar_base_with_3D_printing
- Source date: 31 January 2013.
- Quotes: "First, we needed to mix the simulated lunar material with magnesium oxide." "Then for our structural 'ink' we apply a binding salt which converts material to a stone-like solid." "Our current printer builds at a rate of around 2 m per hour". "A hollow closed-cell structure – reminiscent of bird bones – provides a good combination of strength and weight."
- Confidence: high.
- Use for the ship: a cold, binder-based route (a magnesium oxychloride "Sorel" type cement per other ESA material seen only in search) that suits a magnesium-rich slag powder; it needs a chloride salt supply. No strength figure in the article.

**B25. Cast vs sintered temperatures (1991 NASA-funded study)**
- Numbers: sintering or hot-pressing 1000-1100 °C vs casting 1200-1300 °C.
- Source: Rogers and Sture 1991 (as B13).
- Quote: "While the process te_ is higher than that for sintering or hot-pressing (1000-1100°C), this method is expected to yield a true engineering material with low variability in properties, high strength" (OCR garble "te_" in the original scan).
- Confidence: medium.
- Use for the ship: sintering saves only about 200 °C over full melting but gives a weaker, more variable product.

**B26. Measured strengths of sintered and fused regolith (review of the literature)**
- Numbers (each as reported by the review, citing the original): RegoLight solar-sintered, ambient: 2.49 MPa compressive, 0.21 GPa modulus; JSC-1A sintered in vacuum 152 MPa at 1100 °C vs 98 MPa in air at 1125 °C (Sibille et al.); ICON Laser VMX (with NASA KSC), vacuum-produced: 344.7 MPa at 25 °C, 251.66 MPa after a year of lunar thermal-vacuum cycling, flexural 37.92 MPa; Outward Technologies concentrated-solar powder-bed fusion of CSM-LHT-1: 25 MPa, 10.3 GPa, 95% density, "comparable to M25 concrete"; microwave sintering of KLS-1 in vacuum at 1080-1100 °C: 17-28 MPa (Kim et al.); NASA MMPACT Microwave Structure Construction Capability: a paver of 100% in-situ material up to 1.4 kg, with process work "to minimize energy input in terms of kWh/kg" (no figure given).
- Source: Azami M., Kazemi Z., Moazen S., Dubé M., Potvin M.-J., Skonieczny K., "A Comprehensive Review of Lunar-based Manufacturing and Construction", arXiv:2408.05823v1, https://arxiv.org/pdf/2408.05823
- Source date: 11 August 2024 (preprint).
- Quotes: "Solar sintered materials under ambient atmosphere were characterized by compressive strength of 2.49 MPa and Young's modulus of 0.21 GPa." "the Laser VMX vacuum-produced material showcased high performance, with compressive strengths of 344.7 MPa at 25°C and 251.66 MPa after one year of Lunar thermal vacuum (TVAC) cycles." "vacuum-sintered specimens showcased higher compressive strength at lower temperatures compared to those sintered in the air (152 MPa vs. 98 MPa)."
- Confidence: medium (a review quoting primary work; the ICON figures trace to ICON slides, not a paper).
- Use for the ship: the spread is enormous (2.5 to 345 MPa) and depends on full fusion vs surface sintering and on vacuum. The ship has vacuum for free; vacuum lowers sintering temperature about 100 °C and raises strength. Fully fused/laser-vitrified material rivals cast basalt.

**B27. Microwave heating of regolith: why it is hard to start, easy once hot**
- Numbers: JSC-1AC loss tangent rises from 0.02 at 25 °C to 0.31 at 1100 °C (about the melting point); half-power depth falls from a peak of 286 mm at 110 °C to 13 mm at 1100 °C; sluggish below 200 °C.
- Source: Allan S.M., Merritt B.J., Griffin B.F., Hintze P.E., Shulman H.S., "High Temperature Microwave Dielectric Properties of JSC-1AC Lunar Simulant", preprint for *Journal of Aerospace Engineering*, NTRS 20110007089, https://ntrs.nasa.gov/api/citations/20110007089/downloads/20110007089.pdf
- Source date: 1 January 2011 (NTRS).
- Quotes (from the scanned preprint's OCR text, which renders δ as "8"): "Measurements at 2.45 GHz revealed tan 8 of JSC-1A increases from 0.02 at 25 °C to 0.31 at 1100 °C." "A half-power depth maximum at 100-200 °C presents a barrier to direct microwave heating at low temperatures."
- Confidence: high.
- Use for the ship: microwave sintering couples through iron-bearing, lossy minerals; a slag glass with its iron removed by smelting may couple worse (untested: open question). Taylor and Meek (2005) on Apollo soil not read (ASCE paywall); a 2006 abstract summary credits nanophase iron, while Barmatz et al. (as cited in B26's review) found it may not be the main cause.
- No energy-per-kg figure for microwave or solar sintering was found in any source read. Open question.

---

## 7. Art from slag

**B28. Victorian "slag glass" (marbled pressed glass)**
- Numbers: made in quantity in the 1880s and 1890s in North East England (Sowerby, Greener, Davidson).
- Source: Bowey A.M., "Slag glass", Glass Encyclopedia, https://glassencyclopedia.com/slagglass.html (site serves no robots.txt: 404).
- Source date: undated page, "1998 - 2021".
- Quotes: "The name derives from the belief that these colours were achieved by adding slag from iron smelting works to the glass." "The chemical obtained from iron works was pulverised silicate slag" (fragment). "But using slag in this way was not the only method of making what we now call slag glass."
- Confidence: medium (a well-known collector reference, not a museum record; the V&A collection pages disallow ClaudeBot so were not read).
- Use for the ship: a small, real precedent for decorative ware made with ground slag as a colourant: a crafting item, a token sink, not a tonnage sink.

---

## Addendum to section 1

**B29. Ferronickel slag (a lower-lime analogue) activated with water glass**
- Numbers: blast-furnace ferronickel slag SiO2 29.95, Al2O3 26.31, CaO 25.19, MgO 8.93, Fe2O3 1.55% (table, paraphrased by the fetch); water-quenched; water glass at 10% with modulus 0.5-2.0; best mix (modulus 0.5) 60 MPa compressive, 5.5 MPa splitting tensile; moduli 1.5 and 2.0 had no strength at 1 day. MgO sat mostly in spinel and "magnesium olivine" crystals that barely reacted. Cites Maragkos et al. 2009: electric-furnace ferronickel slag polymers up to 120 MPa.
- Source: Liu K., Liu Z., Sun J., "Effects of the Silicate Modulus of Water Glass on the Hydration and Mechanical Properties of Alkali-Activated Blast Furnace Ferronickel Slag", *Frontiers in Materials* 8 (2021), doi 10.3389/fmats.2021.748833, https://www.frontiersin.org/articles/10.3389/fmats.2021.748833/full
- Source date: 7 September 2021.
- Quotes: "Compared to common GBFS, the CaO content is lower in BFNS." "Because BFNS is generated by water quenching and quick cooling, it has potential for activation." Best result: "60 MPa compressive strength and 5.5 MPa splitting tensile strength." Literature: "the compressive strength could reach 120 MPa with optimal experimental parameters".
- Confidence: high for this slag; it is only 8.93% MgO, so it does not settle the very-high-MgO question.
- Use for the ship: a lime-poor slag still makes a 60 MPa alkali-activated binder if it is quenched to glass; magnesium locked in crystals (spinel, olivine) is inert filler, so quench speed decides how much of an Mg-rich slag is reactive. Next read: Maragkos et al. 2009 (electric-furnace ferronickel slag, high MgO).

---

## Summary of slag per tonne of product (from the entries above)

- Ground granulated slag (cement): about 1 t slag per t; in a Portland blend it is 20-80% of the binder (B7). In alkali-activated binder about 0.93 t slag per t binder plus 55-80 kg Na2O-equivalent activator and about 0.4 t water (B5).
- Concrete with slag binder AND slag aggregate: almost all solids are slag (aggregate B8 plus binder B5); copper-slag aggregate already in hand (Ahmad et al. 2022).
- Crushed aggregate/fill: about 1 t per t, about 2 kWh/t (B12).
- Carbonated compacts: about 0.87-0.91 t slag per t (B10), and they bind 100-150 kg CO2 per t slag.
- Cast slag/basalt: about 1 t per t if the composition suits; 0.36-0.4 MWh/t from cold (B13).
- Slag wool: 0.5-0.8 t per t (B18; NAIMA 70%, B19).
- Continuous fibre: little slag (needs acidity modulus 3-8.5, B16).
- Foamed glass-ceramic: 0.3 t per t in the one recipe read (B22).
- Clear glass: essentially none (B21).
- Sintered blocks: about 1 t regolith per t; slag as the feed is untested in what I read.

---

## Could not read

- https://www.slagcement.org/_files/ugd/... (SCA information sheet PDFs): they redirect to a filesusr.com host whose robots.txt returned 403. Hoped for: SCA info sheets on energy and proportioning. Used the HTML FAQ instead (B7).
- https://pubs.usgs.gov/periodicals/mcs2026/mcs2026-iron-steel-slag.pdf : 403. Hoped for: US slag sales by type and prices (USGS Mineral Commodity Summaries 2026).
- https://www.lafarge.ca/sites/canada/files/images/sca_epd_for_slag_cement.pdf : site redirects to another company domain; not followed. Hoped for: SCA industry-average EPD for slag cement (energy per tonne of ground slag).
- https://mdpi.com/... (carbonation review Materials 17(9) 2066; Materials 2021 slag glass-ceramics; slag wool papers): mdpi.com robots.txt returned 403. The Zhao 2014 paper was read from a re-hosted open copy.
- https://pmc.ncbi.nlm.nih.gov/articles/PMC8123903/ : reCAPTCHA browser check. Hoped for: slag glass-ceramic sintering temperatures and Soviet slag sitall history. Same for PMC9227501 (basalt fibre 3-4 kWh/kg) not attempted after the check.
- europepmc.org, nature.com (Lim et al. 2021, microwave sintering of JSC-1A at 250-1000 W, which may give energy per gram), link.springer.com (Russian electric basalt melting, 1.1 kWh/kg), collections.vam.ac.uk (slag glass objects): robots.txt disallows ClaudeBot / anthropic-ai.
- https://oro.open.ac.uk/ (open copy of Lim et al. 2021): robots.txt returned 403.
- https://core.ac.uk/download/249982377.pdf : 403. Hoped for: basalt fibre melting energy (about 5 kWh/kg per a search snippet).
- https://www.abresist.com/ : TLS certificate expired; not read. Hoped for: fused cast basalt data sheet (density, compressive strength, Mohs).
- https://www.esa.int/gsp/ACT/doc/HAB/ACT-RPR-HAB-2019-EUCASS-Geopolymer.pdf : 404. Hoped for: basalt fibre and geopolymer from lunar regolith.
- yadda.icm.edu.pl (basalt fibre composition study): robots.txt 403.

## Open questions

1. Asteroid slag MgO far above 7.5% (perhaps 20-30% for stony-asteroid smelting without flux): does it still activate with alkali (B6 studied only to 7.5%), and does it hold free periclase that will swell (B9)? Needs a high-MgO slag study (search terms: "high magnesia slag alkali activation", "ferronickel slag geopolymer": ferronickel slag is a real Mg-rich analogue worth reading next).
2. Basicity check for the game: compute (CaO+MgO)/SiO2 and the acidity coefficient (SiO2+Al2O3)/(CaO+MgO) for each asteroid slag recipe. Cement wants basicity > 1 (B4); slag wool wants acidity about 1.2-1.7 (B18); continuous fibre wants 3-8.5 (B16). One slag cannot serve all three without additives: the per-pour choice is a gameplay decision.
3. Microwave and solar sintering energy per kg: no figure found in any source read (Lim et al. 2021 and the MMPACT kWh/kg work are the leads).
4. Fired slag-clay bricks and unfired GGBS bricks: no primary source read (B11).
5. Cast basalt density/strength from a current manufacturer data sheet (only the 1991 NASA table, B13).
6. Slag sitall: a primary Soviet or Russian source (Pavlushkin; Frantsenyuk et al. in full).
7. Ground-slag cement maximum in standards: EN 197-1 CEM III/C allows a very high slag share; not verified from the standard (paywalled).
8. Grinding energy at 4000-5500 cm2/g (the alkali-activation optimum, B5) vs the 3290-3500 Blaine measured in B2: a patent snippet says 40-50 kWh/t at 400-500 m2/kg (not read).

