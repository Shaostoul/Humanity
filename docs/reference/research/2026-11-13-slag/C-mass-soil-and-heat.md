# Slag uses aboard the mothership: research notes (sl-C)

Researched 2026-11-13. Research only; nothing in any repository was edited.
Quotation provenance: quotations marked [pdf] were copied from a `pdftotext -raw` extraction of
the source PDF (exact; line breaks joined). Quotations marked [html] were extracted from the page
by the fetch tool's reader and asked for verbatim; treat them as exact but check against the page
before printing them publicly. "Derived" lines are my arithmetic, never a source's words.
Robots.txt was fetched for every site before any page was read (list at the end).

Ship facts used in derived lines (from the brief, not sources): Drum 1 radius 250 m, water shield
about 10 t/m2, lunar regolith equivalent about 13.4 t/m2 (in hand), thrust cap 0.05 g.

---

## 1. Micrometeoroid protection by a bulk rock layer

**C1. Meteoroid flux at 1 AU (Grun et al. 1985 model, as published by ESA SPENVIS).**
- Numbers: F(m) = 3.15576x10^7 [F1+F2+F3] particles per m2 per year of mass >= m; valid 10^-18 to 1 g.
  F1 = (2.2x10^3 m^0.306 + 15.0)^-4.38; F2 = 1.3x10^-9 (m + 10^11 m^2 + 10^27 m^4)^-0.36;
  F3 = 1.3x10^-16 (m + 10^6 m^2)^-0.85 (m in grams). Recommended constant density 0.5 g/cm3.
- Source: ESA SPENVIS help, "Meteoroids and debris: background", section 2.1.2 The Grun interplanetary flux model. https://spenvis.oma.be/help/background/metdeb/metdeb.html
- Source date: page "Last update: Mon, 12 Mar 2018"; model is Grun et al. 1985 (Icarus), original not read.
- Quote [html]: "F(m) = 3.15576x10^7 [F1(m) + F2(m) + F3(m)]" and the page describes F as "the number of particles per m2 per year of mass larger than or equal to a given mass m."
- Derived (my evaluation of the formula): >=1 mg: 6.0x10^-4 per m2 per yr (one hit per m2 every ~1,700 yr); >=10 mg: 3.1x10^-5; >=0.1 g: 1.5x10^-6; >=1 g: 7.0x10^-8 per m2 per yr = 0.07 per km2 per yr (one per km2 every ~14 yr). Extrapolating past 1 g (outside validity): about a 4 g meteoroid once per km2 per century.
- Confidence: high (formula), medium (the extrapolation).
- Use for the ship: the whole outer shell will see a gram-class hit only every decade or so per km2; hits of 1 mg and below are routine.

**C2. NASA SP-413 (1977) flux and the shield doubling as meteoroid protection.**
- Numbers: >=1 g about once per km2 per 10 years; >=100 g once per 5,000 years; 10 kg once per 100,000 years; typical velocity 40 km/s; a 1 m hole (losing 60% of air takes at least a day) only once in 10,000,000 years.
- Source: Johnson & Holbrow (eds.), NASA SP-413 "Space Settlements: A Design Study", Chapter 2 "Physical Properties of Space", Matter in Space. https://settlement.nss.org/75SummerStudy/Chapt.2.html
- Source date: 1977 (1975 summer study).
- Quotes [html]: "a given square kilometer of space will be traversed by a meteoroid with a mass of 1 g or greater about once every 10 years"; "This radiation shield would also protect against meteoroids."
- Confidence: high (agrees with C1's 0.07 per km2 per yr within a factor of 1.4).
- Use for the ship: SP-413 already concluded that a radiation shield of a few t/m2 makes meteoroids a non-issue; ours is 10 t/m2.

**C3. SP-413 Chapter 2 Appendix A: meteoroids pose little danger to km-sized habitats.**
- Numbers (Table 2-1, partly garbled on the page): 1 g makes a 0.02 m crater; 100 g a 0.09 m crater.
- Source: NASA SP-413, Chapter 2 Appendix A "Meteoroids and Space Habitats". https://settlement.nss.org/75SummerStudy/2appendA.html
- Source date: 1977.
- Quote [html]: "Obviously the hazards of meteoroids pose little danger to kilometer-sized habitats."
- Confidence: high for the conclusion; low for the table values (page text garbled).
- Use for the ship: confirms the design conclusion; craters are cm-scale against a metres-thick shield.

**C4. Crater depth in concrete / processed regolith at 20 km/s (hydrocode, NASA MSFC).**
- Numbers: crater depth D = 4.34 d (d = projectile diameter, stony meteoroid 0.5 g/cm3 at 20 km/s, R^2 = 0.990); wall counted "penetrated" when depth > 1/3 of wall; a 6-inch (15 cm) processed-regolith wall, 330 m2 exposed, 20 years: meteoroid PNP 0.99; critical meteoroid 0.15 g, 0.83 cm.
- Source: Evans, Stallworth, Robinson, Stellingwerf, Engler, "Meteoroid Risk Assessment of Lunar Habitat Concepts", NASA NTRS 20060024733. https://ntrs.nasa.gov/citations/20060024733
- Source date: 2006-01-01 (NTRS).
- Quotes [pdf]: "The crater depths of the models are well represented (R2= 0.990) by the following fit to the data: D = 4.34 * d" ; "The PNP for ejecta is 0.9999, while that due to meteoroids is 0.99, giving an overall PNP of 0.99." ; "For meteoroids the critical case has a mass of 0.15 g and a diameter of 0.83 cm."
- Derived (my arithmetic with D = 4.34 d, density 0.5): 1 g -> 6.8 cm crater; 10 g -> 15 cm; 1 kg -> 68 cm; 10 kg -> 1.5 m. The fit was made on radii 0.125 to 0.75 cm, so the large-mass values are extrapolations.
- Confidence: medium (one validation test on regolith simulant, as the authors say).
- Use for the ship: about 15 to 20 cm of rock-like material stops the gram-class meteoroids a km2 hull sees per decade; the 10 t/m2 shield (10 m of water, or 4.5 to 9 m of rock) is roughly 30 to 60 times thicker than meteoroids need.

**C5. Whipple / bumper shielding (NASA JSC), and solar panels as bumpers (Kalpana One).**
- Numbers: none read (report too large for the reader; direct download timed out).
- Source: Christiansen, "Meteoroid/Debris Shielding", NASA/TP-2003-210788. https://ntrs.nasa.gov/citations/20030068423
- Source date: 2003-08-01.
- Quote [html, abstract]: "This report provides innovative, low-weight shielding solutions for spacecraft" ; "Whipple, Multi-Shock and Stuffed Whipple shield applications are described."
- Second source: Globus, Arora, Bajoria, Strout, "The Kalpana One Orbital Space Settlement Revised" (AIAA), quote [pdf]: "The reinforced solar panels double as micro-meteoroid bumpers."
- Confidence: high that bumpers are standard; no numbers from this pass.
- Use for the ship: the water tanks are a liquid first wall; a thin outer layer (slag tiles or panels) ahead of them stops tank punctures, which is the real meteoroid failure mode for a water shield (leaks), not penetration of the whole 10 t/m2.

**C6. Regolith cover thickness for buried habitats (radiation drives it, not meteoroids).**
- Numbers: about 3 m of regolith for radiation; micrometeoroid shielding provided by the same shell. SP-413's lunar base: 5 m of soil.
- Source A: Giersch, "Archway for Radiation and Micrometeorite Occurrence Resistance" (ARMOR), NASA NTRS 20130009378. https://ntrs.nasa.gov/citations/20130009378 . Date 2012-12-01.
  Quote [html]: "A thickness of roughly 3m is sufficient protection from both direct and secondary radiation"
- Source B: NASA SP-413 Chapter 5, The Site of the Lunar Base, https://settlement.nss.org/75SummerStudy/Chapt5.html (1977). Quote [html]: "It is covered with lunar soil 5 m deep to protect it against meteorites, thermal fluctuations, and ionizing radiation."
- Confidence: medium.
- Use for the ship: every habitat study sizes the cover for radiation and gets meteoroid protection free.

## 2. Mass-driver reaction engine (slag as reaction mass)

**C7. O'Neill's mass-driver reaction engine: 8 km/s, 560 N at 2.9 MW, 75%.**
- Numbers: exhaust 8,000 to 10,000 m/s; 14 g segments of shuttle external tank; 2,100 t/yr reaction mass raises 1,700 t/yr of payload to high orbit; at 8,000 m/s thrust 560 N, power 2.9 MW, efficiency 75%; electrical system mass 89 t.
- Source: O'Neill, G. K., "Mass-driver reaction engine as Shuttle upper-stage", AIAA Paper 77-536, NASA NTRS 19770049208. https://ntrs.nasa.gov/citations/19770049208
- Source date: 1977-05-01.
- Quotes [html, abstract]: "For a mission requiring an exhaust velocity of 8,000 m/sec, thrust is 560 newtons," "power is 2.9 megawatts, and efficiency is 75%." ; "To insure that reaction mass does not constitute a hazard, segments may be in the form of powder," "electrostatically dispersed after acceleration, and/or thrust may be vectored."
- Derived: jet power check 560 N x 8,000 / 2 = 2.24 MW = 77% of 2.9 MW (consistent). Thrust per MW = 193 N/MW. Kinetic energy 32 MJ per kg ejected; electrical 42.7 MJ/kg (11.9 kWh/kg, so 11.9 MWh per tonne of slag thrown).
- Confidence: high (abstract only; full paper not read).
- Use for the ship: slag can be thrown as reaction mass at about 190 N per MW; this is a slow engine (see derived C10).

**C8. SP-428 mass-driver engineering: efficiency definition, 6 to 10 km/s, and the pellet debris hazard.**
- Numbers: high-performance design 6,000 to 10,000 m/s; lunar launcher efficiency 96.4%; reference interorbital engine 8,000 m/s, 0.014 kg payload, 5 Hz; thrust = mass throughput x exhaust velocity.
- Source: Arnold, Bowen, Cohen, Kaplan, Fine, M. Kolm, H. Kolm, Newman, O'Neill, Snow, "Mass Drivers III: Engineering", in NASA SP-428 "Space Resources and Space Settlements". https://settlement.nss.org/spaceres/III-3.html (and Mass Drivers I, https://settlement.nss.org/spaceres/III-1.html)
- Source date: paper 1977-09-23; volume 1979 (NTRS 19790024061).
- Quotes [html]: "may be rendered harmless if it consists of liquid oxygen or a loose aggregate of fine powder" ; "If it is a solid pellet, however, it poses a hazard to other spacecraft as well as to itself" ; "a launch velocity of 8000 m/sec, a payload of 0.014 kg, and a launch rate of 5 Hz"
- Confidence: high.
- Use for the ship: throw slag as loose powder (or a pouch that disperses), never as sintered pellets, so the exhaust cannot become a debris field around the fleet.

**C9. SP-413 rotary pellet launcher and the "allowed pellets" budget.**
- Numbers: 10 g rock pellets, about 2 kg/s at 4,000 m/s, about 200 pellets/s; power system 7,200 t at 45 kg/kW. Pellets ejected faster than Earth-Moon escape but slower than solar escape; Wetherill mean lifetime 10^7 years; allowing one pellet impact per km2 per 10 yr permits 5x10^14 pellets = 5x10^9 t, about 10,000 times the building programme's need.
- Source: NASA SP-413, Chapter 5 Appendix I "Rotary Pellet Launcher" (https://settlement.nss.org/75SummerStudy/5appendI.html) and Chapter 5 "Station-Keeping With the Rotary Pellet Launcher" (https://settlement.nss.org/75SummerStudy/Chapt5.html).
- Source date: 1977.
- Quotes [html]: "To null out, some 2 kg/s are ejected at 4000 m/s." ; "If each has a mass of 10g, the allowed mass of ejected pellets is 5 X 10^9 t." ; "This is some 10,000 times the mass of pellets to be ejected in the course of carrying material for building the colony."
- Derived: thrust 8 kN; jet power 16 MW; 7,200 t / 45 kg/kW = 160 MW of power plant, so roughly 10% overall (my inference; the page's power equation was an image).
- Confidence: medium (SP-413's debris budget is a 1975 argument; SP-428 two years later preferred powder).
- Use for the ship: gives the scale argument that ejected rock is small next to the natural meteoroid flux, but only if it leaves the local system; the safer house rule is C8's powder.

**C10. Rocket-equation comparison (derived, from C7, C9 and C11).**
- Propellant mass fraction 1 - exp(-dv/ve): for 100 m/s: RPL 4 km/s 2.5%; MDRE 8 km/s 1.2%; water resistojet 192 s 5.2%; NEXT ion 4,190 s 0.24%. For 1 km/s: 22% / 12% / 41% / 2.4%.
- Thrust per MW of electricity: MDRE 188 to 193 N/MW; NEXT ion 35 N/MW (237 mN at 6.9 kW).
- Power to hold 0.05 g with an MDRE: 0.49 N per kg of ship / 190 N per MW = about 2.6 MW per tonne of ship. A 1 Mt ship would need about 2.6 TW. At 100 MW, a 1 Mt ship gets about 2x10^-5 m/s2.
- Use for the ship: a slag mass driver is a station-keeping and slow-trim engine (and an attitude or spin-up/despin reaction source), not the 0.05 g manoeuvre engine. Slag consumed per manoeuvre is 1.2% of ship mass per 100 m/s at 8 km/s: bounded by the manoeuvre budget.

**C11. Comparison thrusters (sources for C10).**
- Water resistojet: maximum measured Isp 192 s, thrust 170 to 360 mN, power 443 to 192 W. Morren & Stone, "Development of a liquid-fed water resistojet", NASA NTRS 19880015584, https://ntrs.nasa.gov/citations/19880015584 , 1988-01-01. Quote [html]: "Maximum measured specific impulse was 192 sec."
- NEXT ion: 4,190 s, 237 mN, efficiency 0.71 at 6.9 kW. Herman, Soulas, Patterson, "Performance Evaluation of the Prototype Model NEXT Ion Thruster", NASA NTRS 20080012739, https://ntrs.nasa.gov/citations/20080012739 , 2008-02-01. Quote [html]: "At 6.9 kW, the PM1 thruster demonstrates specific impulse of 4190 s, 237 mN of thrust, and a thrust efficiency of 0.71."
- Survey: Ray, "Characterization of advanced electric propulsion systems", NTRS 19820018505, 1982-05-01. Quote [html]: "Mass drivers have the best performance characteristics in terms of overall efficiency" ; "they can only operate at low specific impulses due to large power requirements"
- Confidence: high.
- Use for the ship: the mass driver sits between steam (cheap, low Isp) and ion (high Isp, tiny thrust per MW), with the unique property that its propellant is waste.

**C12. SP-413: the non-rotating shield as reaction mass for spin-up.**
- Numbers: shield would counter-rotate at about 0.07 rpm if used as the reaction mass during spin-up.
- Source: NASA SP-413 Chapter 5, The View From the Outside. https://settlement.nss.org/75SummerStudy/Chapt5.html (1977).
- Quote [html]: "If the shield is used as a reaction mass during spin-up of the torus it would counter-rotate at approximately 0.07 rpm;"
- Confidence: high.
- Use for the ship: a slag-and-water outer shell is the natural reaction mass for spinning drums up or down, with no propellant spent.

## 3. Ballast and balance of a rotating habitat

**C13. Kalpana One (radius 250 m, like Drum 1): stability rule, wobble control by cable weights, 10 t/m2 shield inside the hull.**
- Numbers: radius 250 m, length 325 m; spin axis moment of inertia at least 1.2 times any other axis (so a flat-capped cylinder length < 1.3 r); up to 10 t/m2 of lunar/NEO regolith inside the hull; total about 7 million t; active wobble control by weights on cables.
- Source: Globus, Arora, Bajoria, Strout, "The Kalpana One Orbital Space Settlement Revised", AIAA paper; PDF http://alglobus.net/NASAwork/papers/2007KalpanaOne.pdf (linked from https://settlement.nss.org/Kalpana/KalpanaOne.html).
- Source date: 2007 (file name; latest reference cited is May 2006; no date printed in the text).
- Quotes [pdf]: "Experience with spin-stabilized spacecraft suggests that the desired axis of rotation should have an angular moment of inertia at least 1.2 times greater than any other axis." ; "Wobble control is provided by weights attached to cables on motorized winches under computer control." ; "Although Kalpana One is rotationally stable, it will tend to wobble since the mass distribution is unlikely to be perfectly uniform and people, machines, and materials will be in constant motion." ; "The size, distribution, and control algorithms for this system would be an excellent thesis topic."
- Also [pdf]: "Contact between these is a catastrophic failure mode not addressed by any of the studies." (rotating habitat a few metres from non-rotating shielding); "Shielding on the interior doubles as soil for plants."
- Confidence: high.
- Use for the ship: solid weights on winched cables are a published balance method, so slag blocks can be the moving counterweights; the paper also flags our design's own risk (spinning drum close to a non-rotating shell) and puts no number on balance accuracy.

**C14. Passive balancers: controller mass 1 to 2% of spacecraft mass.**
- Numbers: four pendulum-like passive controllers; total controller mass need not exceed 1 to 2% of spacecraft mass.
- Source: Martz & Grantham, "A passive balancer for a class of rotating spacecraft", NASA TN D-6924, NTRS 19720026208. https://ntrs.nasa.gov/citations/19720026208
- Source date: 1972-10-01.
- Quotes [html, abstract]: "The ratio of total controller mass to spacecraft mass need not be greater than 1 to 2 percent." ; "passive controllers can successfully balance a class of rotating rigid bodies"
- Confidence: medium (abstract only; the scanned report has no text layer).
- Use for the ship: size the movable balance mass at about 1 to 2% of a drum's mass; it can be slag (solid, on tracks or cables) or water (pumped).

**C15. SP-413: balancing masses around the rim is a layout requirement; shield kept separate and non-rotating.**
- Numbers: shield 9.9 Mt (single torus), 4.5 t/m2, 1.7 m thick, gap about 1.5 m, habitat rim moving at 87 m/s past the shield; "no attention in detail was given" to holding alignment.
- Source: NASA SP-413 Chapter 4 (Shielding) https://settlement.nss.org/75SummerStudy/Chapt4.html and Chapter 5 https://settlement.nss.org/75SummerStudy/Chapt5.html
- Source date: 1977.
- Quotes [html]: "All the architecture within the enclosure of the torus must be distributed to satisfy several requirements:" ... "the need to balance masses around the rim of the torus," ; "the rotating tube would be moving at 87 m/s (194 mph) past the inner surface of the shield" ; "However, no attention in detail was given to this problem."
- Confidence: high.
- Use for the ship: static balance is done first by layout (place fixed slag trim where the layout is light), and only the residual moving load needs active balancing.

**C16. Liquid transfer between tanks as a balance method (spacecraft practice).**
- Numbers: none.
- Source: Hubert (General Electric), US Patent 5,058,834 "Liquid balance control for spinning spacecraft". https://patents.google.com/patent/US5058834A/en . Filed 1989-12-11, published 1991-10-22.
- Quotes [html]: "Whatever the cause of such imbalances, they may adversely affect the pointing accuracy of instruments mounted on the space vehicle." ; on the prior vapour method: "balance control is relatively slow, because the mass transfer rate is low."
- Second source: Wittal, Peterson, Tavarez-Garcia, Kinzel, "Passive Stabilization of Rotating Spacecraft Using Dynamic Fluid-Pressure Equilibrium", NASA NTRS 20230012050 (AIAA/AAS 2023). Quote [pdf]: "of non-potable water alongside the spacecraft to balance the weight and therefore shift the center of mass" (fragment of a sentence beginning "These models demonstrate the relocation").
- Confidence: medium (principle well supported; no numbers for habitat scale).
- Use for the ship: the ship already carries thousands of tonnes of water per drum, so pumping water between rim tanks is the fast, continuous balance medium; slag is the fixed trim and, at most, cable counterweights.

## 4. Thermal mass and heat storage

**C17. Specific heat of lunar soils and basalt: 758 J/kg K at 300 K, rising to about 1,000 J/kg K when hot.**
- Numbers: Hemingway et al. (1973) fit to lunar soils (basalt also measured): cp = -23.173 + 2.127T + 1.5009x10^-2 T^2 - 7.3699x10^-5 T^3 + 9.6552x10^-8 T^4 J/kg K, within 2% to 200 K; 758 J/kg K at 300 K. Higher temperatures (Wechsler et al. 1972): cp = 4184 (0.2029 + 0.0383 (1 - exp((350 - T)/100))).
- Source: Wada, Grott, Michel, Walsh, Barucci, Biele et al., "Asteroid Ryugu Before the Hayabusa2 Encounter", arXiv 1804.03734 (Progress in Earth and Planetary Science). https://arxiv.org/pdf/1804.03734
- Source date: 2018 (written before the June 2018 arrival).
- Quote [pdf]: "Evaluated at a representative average daytime surface temperature of 300 K, a value of cp = 758 J kg-1 K-1 is obtained by evaluating Eq. 14."
- Derived: Wechsler gives 849 J/kg K at 350 K, 973 at 500 K, 1,007 at 800 K. Heating 1 t from 300 K to 1,023 K (750 C) stores about 700 MJ = 0.195 MWh. A 50 K swing near room temperature: about 38 MJ = 10.5 kWh per tonne.
- Confidence: high for 300 K (measured lunar samples); medium above 350 K (extrapolation formula). Slag glass is not identical to lunar soil; iron-rich material has lower cp (the paper notes the iron trend).
- Use for the ship: per tonne, slag buffers about a fifth of what water does near room temperature (C19), so room-temperature buffering belongs to the water shield; slag earns its place as a high-temperature store (hundreds of C), where water would need pressure vessels.

**C18. Siemens Gamesa ETES pilot: 1,000 t of volcanic rock at 750 C stores 130 MWh.**
- Numbers: about 1,000 t rock; heated to 750 C by resistance heater and blower; 130 MWh thermal for up to a week; steam turbine re-electrifies.
- Source: Siemens Gamesa press release, "World first: Siemens Gamesa begins operation of its innovative electrothermal energy storage system". https://www.siemensgamesa.com/global/en/home/press-releases/190612-siemens-gamesa-inauguration-energy-system-thermal.html
- Source date: 2019-06-12.
- Quotes [html]: "contains around 1,000 tonnes of volcanic rock as an energy storage medium." ; "a blower that heats the rock to 750°C." ; "The ETES pilot plant can thus store up to 130 MWh of thermal energy for a week."
- Derived: 0.13 MWh (468 MJ) per tonne usable, against my 0.195 MWh/t theoretical from cold (C17): consistent with a usable swing of roughly 450 to 500 K. Round-trip efficiency not stated.
- Confidence: high (press release; a manufacturer's own figures).
- Use for the ship: a packed bed of crushed slag is a proven heat battery at about 0.13 MWh per tonne; a reactor-load or eclipse swing of 100 MWh thermal needs about 800 t of slag bed.

**C19. Water heat capacity for comparison (NIST).**
- Numbers: liquid water Shomate equation (Chase 1998, NIST-JANAF); at 298.15 K about 75.4 J/mol K, molar mass 18.0153, so about 4.19 kJ/kg K.
- Source: NIST Chemistry WebBook, Water, condensed phase thermochemistry. https://webbook.nist.gov/cgi/cbook.cgi?ID=C7732185&Mask=2
- Source date: page undated; data Chase 1998.
- Quote: table only, no sentence (the reader evaluated the Shomate equation; 75.4 J/mol K is that evaluation, not a printed value).
- Confidence: medium (derived from the equation, not printed).
- Use for the ship: water holds about 5.5 times more heat per kg than rock at room temperature; the 10 t/m2 water shield is already an enormous thermal buffer.

**C20. Molten slag carries about 1.8 GJ per tonne (heat to recover from the smelter).**
- Numbers: cooling from about 1,500 C to ambient releases about 1.8 GJ per tonne; conventional water granulation uses 1,000 L water per tonne and recovers no heat.
- Source: CSIRO, "Dry slag granulation: producing valuable by-products from waste". https://www.csiro.au/en/work-with-us/industries/mining-resources/processing/dry-slag-granulation
- Source date: "Last updated: 02 November 2022".
- Quote [html]: "On cooling from around 1500 °C to ambient temperature, one tonne of molten slag releases about 1.8 gigajoules (GJ) of heat."
- Derived: 1.8 GJ = 500 kWh thermal per tonne of fresh slag.
- Confidence: high.
- Use for the ship: every tonne of slag the smelter makes arrives as a 0.5 MWh heat source; dry granulation (spinning disc, air quench) recovers it and makes glassy granules, which suits a ship that cannot spend 1 t of water per t of slag.

## 5. Soil

**C21. Steel slag as a liming and silicon fertiliser (rice paddy; composition; metals).**
- Numbers: steel slag CaO 40-50%, SiO2 10-28%, MgO 2.5-10%, Fe 17-27%, MnO 1.5-6%; slag pH 11.99; treatments 400 to 2,000 mg plant-available SiO2 per kg soil; soil pH 5.48 rose to 6.43-6.68 at >= 1,600 mg/kg; Cd 0.389, Pb 3.77 mg/kg in the slag.
- Source: Ning, Liang, Liu, Xiao, Duan, "Impacts of Steel-Slag-Based Silicate Fertilizer on Soil Acidity and Silicon Availability and Metals-Immobilization in a Paddy Soil", PLoS ONE 11(12): e0168163. https://pmc.ncbi.nlm.nih.gov/articles/PMC5156425/
- Source date: 2016-12-14.
- Quotes [html]: "Steel slags contain numerous nutrient elements, such as CaO (40–50%), SiO2 (10–28%)" ; "Soil pH increased with increasing steel slag application rate." ; "No significant accumulation of total cadmium (Cd) and lead (Pb) was noted in soil."
- Confidence: high for this study; it did not report Cr or V.
- Use for the ship: steel slag works as lime and silicon; but asteroid-ore slag will not be steel slag (no lime flux, Mg-rich silicate), so treat these as analogues only.

**C22. Chromium and other metal caveats (not read in full).**
- Search found Cai et al. 2023 (High Temperature Materials and Processes, doi 10.1515/htmp-2022-0252) on Cr in stainless-steel slag in soil, and Oliveira et al. 2021 (J. Hazardous Materials) review; neither page was read in this pass. No sourced number.
- Confidence: low (not read).
- Use for the ship: the caveat stands as a design rule to verify: test slag for Cr (and V, Ni from asteroid metal) before it goes into food soil.

**C23. Soil by volume: 45% mineral, 5% organic, 25% water, 25% air.**
- Source: Cornell SoilNOW, "Soil Science 101". https://blogs.cornell.edu/soilnow/the-science-of-soil
- Source date: page undated (footer copyright 2026).
- Quotes [html]: "A soil is approximately 50% solids and 50% pore spaces." ; "In general, minerals make up about 45% of the volume with organic material making up 5%."
- Confidence: high (textbook idealisation of a loam).
- Use for the ship: the mineral fraction of farm soil is 45% of its volume.

**C24. Bulk density: ideal about 1.33 t/m3 at 50% porosity; particle density 2.65.**
- Numbers: ideal porosity 50% = 1.33 g/cm3; particle density 2.65 g/cm3; NRCS table (ideal / root-restricting): loam < 1.4 / > 1.8; clay < 1.1 / > 1.47.
- Source: Klopp & Bly, SDSU Extension, "Bulk Density is an Indicator of Soil Health". https://extension.sdstate.edu/bulk-density-indicator-soil-health
- Source date: updated 2023-09-22.
- Quotes [html]: "Ideal soil porosity is considered to be 50%, which corresponds to a bulk density of about 82.9 lb. ft-3 (1.33 g cm-3)." ; "The particle density of the soil is assumed to be about 165.1 lb ft-3 (2.65 g cm-3),"
- Confidence: high.
- Use for the ship: combined with depth (C25) gives the one-time mineral mass per m2 of farm.

**C25. Farm soil depth and mass in a space settlement design: 0.3 m, 721 kg/m3, 220,000 t.**
- Numbers: crops in lunar soil about 0.3 m deep; Table 5-3: dry soil 1,000,000 m2 x 0.3 m at 721 kg/m3, about 220,000 t (foamed melted rock as growth matrix); 20 m2/person agriculture area in one statement, Table 5-4 about 44.2 m2/person plant crops.
- Source: NASA SP-413 Chapter 5, An Agricultural Area and tables. https://settlement.nss.org/75SummerStudy/Chapt5.html
- Source date: 1977.
- Quote [html]: "The crops are grown in a lunar soil (ref. 4) about 0.3 m deep." Table values: table only, no sentence.
- Derived: one-time mineral mass per m2 of farm: 216 kg/m2 (SP-413 foamed rock, 0.3 m); 358 kg/m2 (loam, 45% x 2.65 t/m3, 0.3 m); 600 kg/m2 at 0.5 m. Kalpana One instead puts its 10 t/m2 regolith shield inside the hull and uses it as soil (C13).
- Confidence: high for the numbers as published.
- Use for the ship: farm soil takes about 0.2 to 0.6 t of slag per m2, once; if the farm sits on the shield (Kalpana style) the soil IS shield mass.

**C26. Annual remineralisation / enhanced weathering: 40 t/ha/yr baseline (Beerling 2020).**
- Numbers: baseline 40 t/ha/yr of basalt, equivalent to a < 2 mm layer, within the range shown to improve crops in field trials; p80 100 um in the global runs; target CDR 0.5 to 2 Gt CO2/yr.
- Source: Beerling, Kantzas, Lomas et al., "Potential for large-scale CO2 removal via enhanced rock weathering with croplands", Nature 583, 242-248, accepted version at https://eprints.whiterose.ac.uk/id/eprint/163140/
- Source date: 2020 (Nature, July 2020).
- Quote [pdf]: "Using this modelling framework, we analysed a baseline application rate of 40 t ha-1 yr-1 (equivalent to a <2 mm layer of rock powder distributed on croplands)"
- Derived: 40 t/ha/yr = 4 kg/m2/yr. Swoboda 2022 (in hand): 1 to 20 t/ha = 0.1 to 2 kg/m2 per application.
- Confidence: high.
- Use for the ship: a closed-loop farm can absorb about 0.1 to 4 kg of rock powder per m2 per year; over a decade the top end adds about 40 kg/m2 (about 3 cm of soil), so the sink is bounded by farm area unless spent soil is moved out (for example, into the shield).

## 6. Other uses that fit a ship

**C27. Steel slag phosphorus filters: 2.15 g P per kg slag.**
- Numbers: EAF steel slag column filters on dairy effluent: average 70% dissolved reactive P reduction over 400 days; retention 2.15 g DRP per kg slag; P fixed as hydroxyapatite and vivianite on the slag; wastewater-to-slag ratio can be set to keep near-neutral pH.
- Source: Lee, Drizo, Rizzo, Druschel, Hayden, Twohig, "Evaluating the efficiency and temporal variation of pilot-scale constructed wetlands and steel slag phosphorus removing filters for treating dairy wastewater", Water Research (2010). http://www.uvm.edu/giee/pubpdfs/Lee_2010_Water_Research.pdf
- Source date: available online 2010-05-24.
- Quotes [pdf]: "EAF steel slag filters achieved an average of 70% dissolved reactive phosphorus (DRP) reduction over a period of 400 days, resulting in P retention capacity of 2.15 g DRP/kg slag (Drizo et al., 2008)." ; "The model also demonstrates how the wastewater:slag ratio can be adjusted to maximize phosphorus removal while staying at a near-neutral pH."
- Confidence: high (column scale, cited from Drizo 2008).
- Use for the ship: a slag bed on the greywater/urine line captures about 2 kg P per tonne, and the P-loaded slag (apatite) is then a soil amendment: a loop, not just a sink (the reuse step itself was not sourced here).

**C28. Mineral carbonation: 81 to 211 kg CO2 per tonne of steel slag (aqueous).**
- Source: Leventaki et al., "Aqueous mineral carbonation of three different industrial steel slags: Absorption capacities and product characterization", Environmental Research 252, 118903. https://research.chalmers.se/en/publication/541270
- Source date: 2024.
- Quote [html]: "reaction yields were in the range of 81–211 kg CO2/ton of slag"
- Confidence: medium (lab reactor; steel slags, not silicate asteroid slag).
- Use for the ship: slag can lock carbon away permanently, which in a closed ship REMOVES carbon from the food loop; useful only as an emergency CO2 scrubber or to bury surplus carbon, and bounded by the ship's carbon budget.

**C29. Slag wool insulation: 78% iron-ore slag; 5 to 8 kg per m2 of wall or ceiling.**
- Numbers: blown mineral wool, 78% iron ore slag + 22% diabase/basalt; about 80% of North American mineral wool made from iron ore slag; R-13 wall 7.9 cm at 64.1 kg/m3 = 5.06 kg/m2; R-38 ceiling 30.6 cm at 27.2 kg/m3 = 8.34 kg/m2; lifetime > 50 years.
- Source: NIST BEES, "Generic Mineral Wool" product data. https://ws680.nist.gov/Bees/ProductListFiles/Generic%20Mineral%20Wool.pdf
- Source date: PDF created 2010-08-13 (metadata).
- Quote [pdf]: "about 80 % of North American mineral wool is manufactured using iron ore slag." Table values: table only, no sentence.
- Confidence: high.
- Use for the ship: slag spun into wool insulates and deadens sound in the habitat; bounded, a few kg per m2 of wall.

**C30. SP-413 used slag itself as the shield, and found the shield swallows all the slag.**
- Numbers: about 10 million t of slag and Moon dust around the habitat; shield bricks of fused lunar soil 1.7 m thick; shielding needs exceed the processing plant's slag output.
- Source: NASA SP-413 Chapter 4 (Fabrication Techniques; Processing: Where?) https://settlement.nss.org/75SummerStudy/Chapt4.html and Chapter 5 https://settlement.nss.org/75SummerStudy/Chapt5.html
- Source date: 1977.
- Quotes [html]: "Large pieces of shield are placed around the completed portions as the slag material becomes available" "from the processing plant." ; "However, closer examination reveals that the colony's shielding requirements exceed the slag production" "of the processing plant; hence, no transportation is saved by processing at a lunar site." ; "But still the 10 million tonnes of slag and Moon dust that have been compacted and placed around the habitat"
- Confidence: high.
- Use for the ship: the one sink that grows without limit is NEW shield for new habitat: each new drum or ship at 10 to 13.4 t/m2 absorbs far more than the farms, balancers or heat stores ever will.

---

## Could not read
- https://pubs.usgs.gov/of/1988/0441/report.pdf and https://pubs.usgs.gov/publication/ofr95622 : HTTP 403 (refused). Hoped for Robertson 1988 table of rock specific heats and Robertson & Hemingway 1995 basalt heat capacity.
- https://polarnightenergy.com/ : robots.txt disallows ClaudeBot; not read. Hoped for the Pornainen sand battery (search snippets said 100 MWh, 1 MW, 500-600 C, crushed soapstone; unverified).
- https://nottingham-repository.worktribe.com/ : robots.txt disallows ClaudeBot and anthropic-ai; not read. Hoped for basalt cp(T) data.
- https://extension.okstate.edu/ : robots.txt returned 403; not read (soil bulk density fact sheet).
- https://www.nrcs.usda.gov/robots.txt : timed out twice; NRCS pages not read (soil bulk density guide, "Conserving Soil" composition).
- https://orca.cardiff.ac.uk/ : robots.txt 403; not read (Beerling 2025 Nature, US cropland ERW).
- https://doaj.org/article/14648fc5f1964a75810b6f82a45e4862 : 403. Hoped for Murujew et al. 2024 H2Open full-scale BOF slag wetland, 1.39 mg P/g after 782 days (search snippet only, unverified).
- https://www.e3s-conferences.org/... (rock TES) : 403. https://diposit.ub.edu/... : 404.
- https://ntrs.nasa.gov/api/citations/20030068423/downloads/20030068423.pdf (Christiansen 2003): too large for the reader; direct download timed out. Hoped for Whipple ballistic limit equations and the bumper-versus-monolithic mass saving.
- https://repository.tudelft.nl/file/File_8aaebf98-47e8-40c2-b0bf-2053101de7aa : too large for the reader. Hoped for the ESA/TU Delft lunar base: 1.2 m regolith + 4 cm water, meteoroid PNP above the 99.8% requirement (search snippet only).
- https://gsa.confex.com/... (GSA 2022 abstract citing Lindsey 2003, 1 m regolith for meteoroids): robots.txt redirected to another host, so permission unclear; not read.
- NASA TN D-1515 (Adams 1962, active control of a spinning station), TN D-2749 (Polstorff 1965, rotating cylindrical station dynamics), TN D-5426 (Larson 1969, stabilization criteria), TN D-6924 full text: scanned PDFs with no text layer; only NTRS metadata (and the D-6924 abstract) read. Hoped for required balance accuracy and wobble limits.
- Grun et al. 1985 (Icarus) original and Snow, Dunbar, Kubby, O'Neill 1982: not located in a readable copy; Grun used via SPENVIS (C1), Snow et al. not used.
- Ortega-Fernandez et al. 2015 (Energy 89, EAF slag for TES up to 1,000 C): not read.
- Basic (Thomas) slag P2O5 content: no primary source found in this pass.

## Open questions
1. What slag will our smelter actually make? Asteroid ore (chondritic silicates, Fe-Ni metal) smelted without lime flux gives Mg-Fe silicate slag, not CaO-rich steel slag; most soil, P-filter and carbonation numbers above are steel-slag analogues. Cr, Ni, V content must be decided per ore type.
2. Required balance accuracy for a 250 m drum (residual wobble a person tolerates) was not found in any readable source; Kalpana One calls it a thesis topic. The 1960s NASA notes (scanned) are the likely source if OCR is allowed.
3. The non-rotating outer shell with spinning drums inside is exactly the "catastrophic failure mode" Kalpana One warns of (contact between a rotating hull and nearby non-rotating shield); the gap and positioning system need design (SP-413: 1.5 m gap, "positive positioning device", details not studied).
4. Whether P-loaded slag from a filter is safe and effective as fertiliser (the loop in C27) is unsourced.
5. Mass-driver exhaust in a jump-cruise game: if the ship leaves the system by jump, ejected powder stays behind in the departure system; SP-428's powder rule still applies near other ships and stations.
6. Endless sink: only two uses are unbounded over time, new shield for an expanding fleet (C30) and throwing slag overboard (C7 to C10); every other use is a one-time fill or saturates (soil, balance trim, heat store, insulation, filters).
