# Slag aboard the mothership: research notes (helper A)

Researched 2026-11-13. Research only; nothing in any repository was edited.

How the quotations were checked: "pdftotext" means I extracted the PDF text myself and copied
the words from it exactly. "page reader" means the words came through WebFetch's page reader. It
quotes the page but I could not compare it byte for byte, so check those before they go into a
dated finding. Every figure marked DERIVED is my own arithmetic on sourced inputs. The script is
[residue-per-tonne.js](residue-per-tonne.js) in this folder (`node residue-per-tonne.js`)
and the inputs are named in each entry.

---

## 1. Bulk composition of the three asteroid types

**A1. Chondrite bulk element compositions (CI, CM, H, L, LL), ppm by mass**
- Numbers (Table 1, columns CI / CM / H / L / LL):
  - H: 18600 / 11500 / 800 / 1260 / 1700
  - C: 41300 / 23200 / 2350 / 2500 / 3920
  - Mg: 95170 / 119000 / 139400 / 148600 / 153000
  - Al: 8370 / 11400 / 11100 / 11700 / 11800
  - Si: 107740 / 132000 / 170400 / 184600 / 189000
  - S: 53600 / 30000 / 20000 / 22000 / 21000
  - Ca: 8840 / 12000 / 12400 / 13000 / 13200
  - Na: 5100 / 4000 / 6100 / 6900 / 6900
  - Fe: 185620 / 211000 / 271500 / 218450 / 198000
  - Ni: 10950 / 12500 / 17100 / 12700 / 10200
  - Co: 508 / 560 / 805 / 600 / 480
  - Pt: 0.931 / 1.1 / 1.58 / 1.08 / 0.88
  - Ir: 0.474 / 0.59 / 0.76 / 0.52 / 0.36
  - Os: 0.475 / 0.65 / 0.83 / 0.56 / 0.39
  - Pd: 0.558 / 0.63 / 0.845 / 0.62 / 0.51
  - Ru: 0.666 / 0.86 / 1.1 / 0.75 / 0.55
  - Rh: 0.133 / 0.16 / 0.22 / 0.155 / 0.11
  - Au: 0.147 / 0.152 / 0.22 / 0.16 / 0.14
- Source: K. Lodders, "Relative Atomic Solar System Abundances, Mass Fractions, and Atomic Masses
  of the Elements and Their Isotopes, Composition of the Solar Photosphere, and Compositions of the
  Major Chondritic Meteorite Groups", Space Science Reviews 217:44. Accepted manuscript on the NSF
  Public Access Repository: https://par.nsf.gov/servlets/purl/10642789 (I did not use Springer:
  its robots.txt disallows ClaudeBot and anthropic-ai).
- Source date: received 18 February 2021, accepted 24 March 2021 (2021).
- Quote (pdftotext): "Table 1 gives the elemental composition by mass of the major chondrite groups in parts per million (µg/g)." The values themselves are table only (Table 1, no sentence).
- Confidence: high. These are group means drawn from several hundred analyses. Lodders gives
  elements, not oxides or phases.
- Use for the ship: the canonical per-type element budget. The oxide, water and troilite figures
  below are derived from it.

**A2. DERIVED from A1: oxides, water-equivalent and troilite**
- Numbers, the non-iron oxides summed (SiO2+MgO+Al2O3+CaO+Na2O+K2O+Cr2O3+MnO+TiO2+P2O5):
  - CI 43.3 wt%, CM 53.4, H 65.5, L 70.4, LL 72.1.
- Water-equivalent if all the H sits in H2O: CI 16.6 wt%, CM 10.3 (an upper bound, since some H is
  in organics).
- Carbon: CI 4.13 wt%, CM 2.32, H 0.23, L 0.25, LL 0.39.
- Troilite FeS if all the S sits in it: H 5.5 wt%, L 6.0, LL 5.8 (CI 14.7 and CM 8.2 are only
  formal: their S is partly in sulfates and elemental S).
- Oxide forms for H: SiO2 36.4, MgO 23.1, Al2O3 2.10, CaO 1.73, Na2O 0.82 wt%.
- Source: arithmetic on A1 with standard molar masses.
- Confidence: medium. The arithmetic is exact; the phase assumptions are approximations.
- Use for the ship: oxide inputs for slag recipes when no oxide table can be read (Jarosewich 1990
  could not be read, see "Could not read").

**A3. Ordinary chondrite metal, by volume, and total Fe**
- Numbers (equilibrated chondrites, H / L / LL):
  - metal 8.4 / 4.1 / 2 vol%
  - bulk Fe 28 / 22 / 19 wt%
  - falls 34 / 37 / 9 %
- Source: P. Vernazza, B. Zanda, T. Nakamura, E. Scott, S. Russell, "The Formation and Evolution of
  Ordinary Chondrite Parent Bodies", a chapter of Asteroids IV (University of Arizona Press),
  arXiv:1611.08734, https://arxiv.org/pdf/1611.08734
- Source date: 26 November 2016 (arXiv v1). The book is 2015/2016.
- Quote (pdftotext): "They are « Low » in total iron, « Low » in metal, hence the LL denomination." The numbers are table only (Table 1, "Properties of equilibrated ordinary chondrites"). In the extracted text the row labels are shifted one row against the values. I re-aligned them by meaning: fall percentages sum to 80, and total Fe follows H > L > LL. Check against the PDF page.
- Confidence: medium, because of the realignment.
- Use for the ship: how much free metal an S-type rock holds.

**A4. DERIVED from A3: metal by weight**
- Numbers: H about 17.0 wt% metal, L about 8.3, LL about 4.1.
- Method: vol% times metal density over rock density. I took mid-points of the Ross 2001 Table 3
  density ranges (A6): metal 7.4, S-type rock 3.65 g/cm3.
- Cross-check: Kargel's abstract gives LL as 1.2-5.3% metal (A9), and Ross gives S-type free Fe as
  6-19% (A6).
- Confidence: medium.
- Use for the ship: the metal-grain fraction behind the "residue per tonne of metal" answers.

**A5. Average iron meteorite (M-type)**
- Numbers:
  - Fe 90.6, Ni 7.9, Co 0.5 wt%; Cu 150 ppm
  - Ru 20.7, Rh 3.9, Pd 2.6, Os 14.1, Ir 14.0, Pt 28.8, Au 0.6 ppm
  - PGM and Au are at a "good 90th percentile in Ir and Pt"
- Source: J. A. Sanchez, V. Reddy, W. F. Bottke et al., "Physical Characterization of Metal-rich
  Near-Earth Asteroids 6178 (1986 DA) and 2016 ED85", Planetary Science Journal (submitted
  version), arXiv:2109.13950, https://arxiv.org/pdf/2109.13950
- Source date: draft of 30 September 2021. It cites Buchwald (1975) for Fe, Ni, Co and Cu, and
  Kargel (1994, 1996) for PGM and Au.
- Quote (pdftotext): "Abundances corresponding to Fe, Ni, Co, and Cu in iron meteorites are from Buchwald (1975). The abundances of Au and the PGM are from Kargel (1994, 1996)". The numbers are table only (Table 5).
- Confidence: high for Fe, Ni and Co. Medium for PGM, which is a rich-end case, not the median.
- Use for the ship: the M-type metal recipe.

**A6. Four asteroid recipes, after O'Leary et al. 1979**
- Numbers, Table 3 (C2-type / C1-type / S-type / M-type):
  - free Fe: 10.7 / 0.1 / 6-19 / 88 %
  - Ni: 1.4 / -- / 1-2 / 10 %
  - Co: 0.11 / -- / 0.1 / 0.5 %
  - C: 1.4 / 1.9-3.0 / 3 / -- %
  - H2O: 5.7 / 12 / 0.15 / -- %
  - S: 1.3 / 2 / 1.5 / -- %
  - FeO: 15.4 / 22 / 10 / -- %
  - SiO2: 33.8 / 28 / 38 / -- %
  - MgO: 23.8 / 20 / 24 / -- %
  - Al2O3: 2.4 / 2.1 / 2.1 / -- %
  - Na2O: 0.55 / 0.3 / 0.9 / -- %
  - K2O: 0.04 / 0.04 / 0.1 / -- %
  - P2O5: 0.28 / 0.23 / 0.28 / -- %
  - CaO: not listed for any asteroid type
  - density: 3.3 / 2.0-2.8 / 3.5-3.8 / 7.0-7.8 g/cm3
- Source: Shane D. Ross, "Near-Earth Asteroid Mining", Space Industry Report (Caltech),
  https://ross.aoe.vt.edu/papers/ross-asteroid-mining-2001.pdf
- Source date: 14 December 2001.
- Quotes (pdftotext):
  - "one can estimate that the non-carbonaceuous asteroids contain the following: about 20% metallic iron-nickel alloy; about 6% of the ferrous sulphide mineral troilite"
  - "Iron meteorites, or metallic M-type asteroids, are even more metal-rich; about 99% metal."
- Confidence: medium. These are single example meteorites from 1979. The "C2" column's 10.7% free Fe
  does not match modern CM data (A1 gives CM as mostly oxidised Fe). Use A1 for the C-types.
- Use for the ship: the oft-quoted round numbers (S-type about 20% metal plus 6% troilite; M-type
  about 99% metal) and the only readable table with oxides per asteroid type.

**A7. C-type water and carbon, in words**
- Numbers:
  - C1: about 10% water, 5-15% hydration salts, 2-5% carbon, 5-15% magnetite
  - C-types overall: 5-20% water
- Source: Ross 2001 (as in A6).
- Quotes (pdftotext):
  - "C1 carbonaceous chondrites average about 10% water in a clay mineral matrix and as water of hydration (often in magnesium salts, 5% to 15%), 2% to 5% carbon"
  - "C-type asteroids and carbonaceous meteorites typically have 5% to 20% water."
- A second source: NASA SP-509 Space Resources vol. 3 (1992), the "Meteorite Classes" page,
  https://settlement.nss.org/spaceresvol3/asterres1b.htm (page reader). It gives C1 as "up to 20
  percent water chemically bound in clay minerals", "up to 6 percent organic matter" and "up to 11
  percent magnetite".
- Disagreement: Lodders' H-derived water-equivalent for CI is 16.6 wt% (A2, an upper bound).
- Confidence: medium.
- Use for the ship: C-type volatile yield before smelting. The water and carbon leave as gas, so
  they are not slag.

**A8. PGM in iron meteorites: spread and terrestrial comparison**
- Numbers:
  - Ir in irons spans 0.01-100 ppm, and total PGM is about 7 times Ir
  - IIIAB irons, all PGMs by percentile: 90th 75 ppm, 50th 6.8 ppm, 10th 0.17 ppm
  - good terrestrial PGM mines run 2-6 ppm
  - about 4% of NEOs are metallic (Ptype)
- Source: M. Elvis, "How Many Ore-Bearing Asteroids?", Planetary and Space Science 91 (2014),
  doi 10.1016/j.pss.2013.11.008, arXiv:1312.4450, https://arxiv.org/pdf/1312.4450
- Source date: submitted 16 December 2013.
- Quotes (pdftotext):
  - "Ir concentrations span four orders of magnitude, from 0.01 to 100 parts per million (ppm, Kargel 1994)."
  - "Good terrestrial mines have PGM concentrations of up to 2--6 ppm"
  - The percentile values are table only (Table 1).
- Confidence: high, as a summary of Kargel 1994 and Scott, Wasson and Buchwald 1973.
- Use for the ship: M-type metal is PGM ore at the median and rich ore at the top decile.

**A9. Kargel 1994 abstract: LL metal and precious metals**
- Numbers:
  - LL chondrites carry 1.2-5.3% Fe-Ni metal, holding 50-220 ppm precious metals
  - metallic asteroids hold "up to several hundred ppm"
  - about 6 metallic NEAs larger than 1 km hold over 100 ppm
- Source: J. S. Kargel, "Metalliferous asteroids as potential sources of precious metals", Journal of
  Geophysical Research 99(E10):21129 (1994). Abstract read on a mirror,
  https://stafforini.com/works/kargel-1994-metalliferous-asteroids-potential/ (page reader). The
  journal page was not tried; Wiley/AGU.
- Source date: 1994.
- Quote (page reader): "the ordinary LL chondrites, which contain 1.2-5.3% Fe-Ni metal containing 50-220 ppm of precious metals" (here "precious metals" = Au, Pt, Ir, Os, Pd, Rh, Ru).
- Confidence: medium. The abstract came via a mirror; check it against JGR.
- Use for the ship: metal from oxidised stony asteroids is scarce but rich.

**A10. A second "90th percentile" iron, which disagrees with A5**
- Numbers: Ru 13.0, Rh 4.8, Pd 1.3, Re 3.7, Os 9.0, Ir 33.0, Pt 35.0, Au 0.5 ppm.
- Source: Ross 2001, Table 2, "Taken from Kargel [1997]".
- Quote (pdftotext): "The asteroid is assumed to be a 1 km metallic asteroid (90th percentile in iridium richness)." The numbers are table only (Table 2).
- Disagreement: A5 gives Ir 14.0 and Pt 28.8 for the same "90th percentile" label. Ross also
  quotes "up to 187 parts per million (ppm) of precious metals" for irons.
- Confidence: medium.
- Use for the ship: give the rich iron a range (Pt about 29-35 ppm, Ir about 14-33 ppm), not one
  number.

**A11. Mesosiderites and related stony-irons (the middle case)**
- Numbers:
  - mesosiderites: roughly equal silicate and metal
  - pallasites: metal and olivine in roughly equal amounts
  - bencubbinites: about 60-80 vol% metal
  - CH chondrites: about 20 vol% metal
  - the metal-rich NEA 1986 DA fits as about 85% metal and 15% pyroxene
- Source: Sanchez et al. 2021 (as in A5).
- Quote (pdftotext): "Mesosiderites are breccias composed of similar proportions of silicates and FeNi metal."
- Confidence: medium; the proportions are qualitative.
- Use for the ship: the stony-iron case leaves about 1 t of residue per t of metal (A14).

---

## 2. Slag (or residue) per tonne of metal

**A12. Definitions: slag vs tailings**
- Slag, per USGS (pdftotext): "Iron and steel (or ferrous) slags are silicate melts that result from the addition of slagging agents and fluxes (chiefly limestone or dolomite, lime, and silica sand)"
  - Source: H. G. van Oss, USGS 2017 Minerals Yearbook, "Slag--Iron and Steel" (advance release),
    https://d9-wret.s3.us-west-2.amazonaws.com/assets/palladium/production/atoms/files/myb1-2017-fesla.pdf
  - Source date: May 2020.
- Slag, per EPA (page reader): "Byproduct left after a desired metal is thermally separated from its raw ore."
- Tailings, per EPA (page reader): "The remaining portion of a metal-bearing ore after some or all of a metal, such as uranium, has been extracted."
  - Source for both EPA entries: US EPA, "TENORM: Copper Mining and Production Wastes",
    https://www.epa.gov/radiation/tenorm-copper-mining-and-production-wastes
  - Source date: page last updated 29 September 2026.
- Slag, per FHWA (page reader): "Blast furnace slag is a nonmetallic coproduct produced in the process. It consists primarily of silicates, aluminosilicates, and calcium-alumina-silicates."
  - Source: FHWA-RD-97-148, see A16.
- Confidence: high.
- Use for the ship: the residue is slag only if the ore is melted. Metal grains pulled out of
  crushed, unmelted rock (magnetic separation, carbonyl) leave tailings. Name the game item by the
  process.

**A13. Terrestrial reference ratios (the in-hand worldsteel figure is not repeated)**
- Numbers:
  - blast furnace: 0.25-0.30 t of slag per t of pig iron at 60-66% Fe ore, and up to 1.0-1.2 t/t
    for low-grade ore
  - steel furnaces: about 0.2 t/t, up to 50% of it entrained metal, so 10-15% of crude steel is
    marketable slag
- Source: USGS 2017 Minerals Yearbook (as in A12).
- Quote (pdftotext): "a blast furnace normally will produce about 0.25 to 0.30 metric ton (t) of slag per ton of crude or pig iron produced."
- Second source: FHWA (page reader): "The molten slag, which absorbs much of the sulfur from the charge, comprises about 20 percent by mass of iron production."
- More from EPA on copper (page reader): "Approximately 75 percent of the copper concentrate ends up as slag." EPA also gives 2.5 Mt of smelter slag and 1.5 Mt of slag tailings per year (US).
- Confidence: high.
- Use for the ship: Earth's blast furnace makes about 0.3 t of slag per t of iron because the ore is
  60-66% Fe. Every asteroid type except M is far leaner than that (A14).

**A14. DERIVED: residue or slag per tonne of metal, by asteroid type**
- S-type, metal grains only (crush, separate, melt only the metal):
  - H about 4.9 t of residue per t of metal, L about 11, LL about 24 (from A4)
  - Ross's round "20% metal" gives 4.0 t/t
  - Unmelted, this residue is tailings. Melted, it is slag.
- S-type, all iron reduced (FeO and FeS iron too):
  - H 2.26 t of slag per t of metal, L 3.04, LL 3.46
  - This is the non-iron oxides of A2 divided by Fe+Ni+Co from A1.
  - It assumes complete reduction of FeO and removal of S; Si, Mg, Al and Ca stay as oxide.
- C-type:
  - free metal only: about 1,000 t of residue per t of metal (Ross's C1 free Fe 0.1%)
  - all iron reduced: CI 2.20 t/t, CM 2.38 t/t
  - the water and carbon (16.6 and 4.13 wt% for CI) leave as gas first
  - CI carbon (0.344 mol per 100 g) is about stoichiometric with its Fe (0.332 mol per 100 g), so
    the rock carries roughly its own reductant. This is arithmetic only; how much of the organic
    carbon is usable as reductant is open.
- M-type:
  - about 10-15 kg per t of metal (Ross's 99% metal, or 98.5% Fe+Ni+Co in Table 3)
  - the residue is troilite, phosphide and silicate inclusions; their split is not sourced here
- Stony-iron (mesosiderite): about 1 t per t of metal (A11's "similar proportions").
- Source: arithmetic on A1, A3, A4, A6 and A11.
- Confidence: medium (the inputs are high to medium).
- Use for the ship: the slag-per-tonne table for the design. It is per tonne of iron-group metal,
  before any oxide electrolysis (section 4).

---

## 3. Terrestrial slag compositions, and what asteroid slag would be

**A15. Blast-furnace slag composition, means by survey year (wt%)**
- Numbers (1985 survey, mean and range):
  - CaO 39 (34-43)
  - SiO2 36 (27-38)
  - Al2O3 10 (7-12)
  - MgO 12 (7-15)
  - FeO or Fe2O3 0.5 (0.2-1.6)
  - MnO 0.44 (0.15-0.76)
  - S 1.4 (1.0-1.9)
- For comparison, 1949: CaO 41, SiO2 36, Al2O3 13, MgO 7.
- Source: FHWA, "User Guidelines for Waste and Byproduct Materials in Pavement Construction",
  FHWA-RD-97-148, Blast Furnace Slag chapter,
  https://www.fhwa.dot.gov/publications/research/infrastructure/structures/97148/bfs1.cfm
  (page reader)
- Source date: report 1997/1998. The page says it was last modified 2016 and is marked archive.
- Quote: table only (Table 3-2, typical composition of blast furnace slag). Volume sentence (page reader): "approximately 14 million metric tons (15.5 million tons) of blast furnace slag is produced annually in the United States."
- Confidence: high.
- Use for the ship: the reference "slag" a player knows. Its CaO comes from the limestone and
  dolomite flux (A12).

**A16. Steel (BOF/EAF) slag composition (wt%)**
- Numbers:
  - CaO 40-52
  - SiO2 10-19
  - FeO 10-40 (70-80% FeO, 20-30% Fe2O3)
  - MnO 5-8
  - MgO 5-10
  - Al2O3 1-3
  - P2O5 0.5-1
  - S <0.1
  - metallic Fe 0.5-10
- Source: FHWA-RD-97-148, Steel Slag chapter,
  https://www.fhwa.dot.gov/publications/research/infrastructure/structures/97148/ssa1.cfm (page
  reader)
- Source date: 1997/1998; the page was modified 03/08/2016.
- Quote: table only (Table 18-2, typical steel slag chemical composition). Partial sentence (page reader): "Steel slag, a by-product of steel making, is produced during the separation of the molten steel" from impurities in steel-making furnaces.
- Confidence: high.
- Use for the ship: a second reference, iron-rich because some iron is lost to the slag.

**A17. DERIVED: asteroid smelting slag with no flux (wt%, normalised non-iron oxides from A1)**
- Numbers:
  - H: SiO2 55.6, MgO 35.3, Al2O3 3.2, CaO 2.6, Na2O 1.3, Cr2O3 0.8, MnO 0.5
  - L and LL: nearly identical (SiO2 56.1, MgO 35.0-35.2, Al2O3 3.1, CaO 2.6)
  - CI: SiO2 53.2, MgO 36.4, Al2O3 3.7, CaO 2.9
  - CM: SiO2 52.9, MgO 37.0, Al2O3 4.0, CaO 3.1
- Before full reduction, add FeO: Ross gives S-type FeO 10% and C1 FeO 22% of the rock (A6).
- Contrast with A15: CaO/SiO2 is about 1.1 in blast-furnace slag and about 0.05 in asteroid slag.
  Asteroid slag is a magnesium silicate (olivine/pyroxene, close to an ultramafic rock), not a
  lime-alumina silicate.
- Source: arithmetic on A1.
- Confidence: medium.
- Use for the ship: the "asteroid slag" item recipe. If cement-like (lime-rich) slag is wanted, lime
  flux must be brought or made.

**A18. Lunar soil already resembles slag**
- Quote (pdftotext): "Overall, the lunar surface is volatile-poor and metal-poor, similar in composition to the slag discarded in metallurgical processing on Earth (Lewis [1997])."
- Source: Ross 2001 (as in A6).
- Confidence: medium; it is a qualitative statement.
- Use for the ship: lunar-regolith processing numbers (section 4) are a fair stand-in for "processing
  silicate slag".

---

## 4. Getting more out of slag

**A19. Molten regolith electrolysis (MRE): 2026 NASA 25 kg demonstration**
- Numbers:
  - 25 kg of regolith; electrolysis for 9 h at 45 V and 210 A
  - produced oxygen 2.3 wt% of the charge
  - 0.07 kg/h average
  - "Oxygen Production Efficiency: 0.3 Mols O2/kW-hr"
  - melt sustained above 1700 °C by Joule heating; up to 80 vol% O2 in the off-gas
  - DERIVED: 0.3 mol/kWh is about 104 kWh per kg of O2. 9.45 kW over 0.07 kg/h is about 135 kWh/kg.
- Source: J. A. Toro Medina, A. Meier, M. Shah, E. Bell, D. Essumang, J. A. Olson, L. Sibille,
  "Large-scale Demonstration of Molten Regolith Electrolysis for Oxygen Production in a Relevant
  Environment" (ICES 2026 poster), NASA KSC, NTRS 20260006167,
  https://ntrs.nasa.gov/api/citations/20260006167/downloads/MRE%20GCD%20ICES26_v3.pdf
- Source date: 2026 (ICES 2026).
- Quote (pdftotext): "Produced Oxygen 2.3 Weight % / Average Production Rage: 0.07 kg/hr / Oxygen Production Efficiency: 0.3 Mols O2/kW-hr" (bullets on a poster; the typo "Rage" is in the source).
- Confidence: high for what was measured. It is a first large demonstration, not an optimised plant.
- Use for the ship: present-day measured MRE costs about 100-135 kWh per kg of O2.

**A20. MRE whole-system model (MIT/NASA)**
- Numbers:
  - 400 kg and 14 kW make 1,000 kg of O2 a year
  - 1,593 kg and 56.5 kW make 10,000 kg a year
  - DERIVED, if run continuously: about 123 and about 49.5 kWh per kg of O2, an economy of scale
- Source: S. S. Schreiner, "Molten Regolith Electrolysis reactor modeling and optimization of
  in-situ resource utilization systems", S.M. thesis, MIT, https://dspace.mit.edu/handle/1721.1/98589
  (abstract, page reader).
- Source date: 2015.
- Quote (page reader): "The optimized models predict that a 400 kg, 14 kW MRE-based ISRU system can produce 1,000 kg oxygen per year"
- Confidence: medium (a model). The whole-system figure includes excavation and liquefaction.
- Use for the ship: the planning range for a mature MRE plant is about 50-125 kWh per kg of O2.

**A21. MRE current efficiency and temperature**
- Numbers:
  - instantaneous current efficiency is 30-60% while iron oxides are being reduced, and near 100%
    once FeO is depleted
  - reactor at 1900 K for small plants and 2200 K for large ones (several reactors of 2,500 kg/yr
    each)
- Source: S. S. Schreiner, J. A. Hoffman, G. B. Sanders, K. A. Lee, "Integrated Modeling and
  Optimization of Lunar In-Situ Resource Utilization Systems", 2015 IEEE Aerospace Conference
  (978-1-4799-5380-6/15),
  https://sciences.ucf.edu/class/wp-content/uploads/sites/23/2017/01/Muscatello-Integrated-modeling-and-optimization-of-lunar-In-Situ-Resource-Utilization-systems-MRE.pdf
- Source date: 2015.
- Quote (pdftotext): "can range from 30-60% for iron oxides and is near 100% for melts once the iron oxide has been depleted"
- Second source, secondary: M. Azami et al., "A Comprehensive Review of Lunar-based Manufacturing
  and Construction", arXiv:2408.05823 (11 August 2024). It reports the Schreiner et al. 2016 reactor
  model. Quote (pdftotext): "For Highland regolith, increasing temperatures enhance current
  efficiency from 72.5% to 82%, while for Mare regolith, efficiencies start at 57.5% and rise to 70%".
  Its oxygen yield is about 0.15-0.375 kg of O2 per kg of regolith.
- Temperature, primary: Sirk, Sadoway and Sibille 2010, ECS 217th meeting, NTRS 20110008535.
  Quote (pdftotext): "The silicate mixture (with compositions and mechanical properties corresponding to that of lunar regolith) is melted at temperatures near 1600 'c." Its iridium anode is shown in Fig. 2. Products: "primarily iron and silicon at the cathode".
- Confidence: high (primary) for the 30-60 / ~100% split. Medium for the 57.5-82% batch averages
  (seen only through the review).
- Use for the ship: iron-rich slag electrolyses at lower efficiency. Electrolysing slag after the
  metal is taken costs fewer kWh per kg of O2.

**A22. DERIVED: the Faraday floor for oxygen by electrolysis**
- Numbers: 4 electrons per O2 means 3,350 Ah per kg of O2, which is 3.35 kWh per kg per volt of cell
  voltage at 100% current efficiency. (The 45 V in A19 includes the Joule heating that keeps the
  melt molten.)
- Source: arithmetic. The "4 electrons" is stated in A19: "Each oxygen molecule formed requires removal of 4 electrons."
- Confidence: high.
- Use for the ship: a sanity bound for any slag-electrolysis power number.

**A23. FFC-Cambridge (molten-salt) electrolysis of solid regolith**
- Numbers:
  - 96% of the oxygen extracted, at about 950 °C in molten CaCl2 with the regolith kept solid
  - products: Al/Fe(-Si), Fe/Si(-Ti/Al) and Ca/Si/Al(-Mg) alloys
- Sources:
  - Lomax, Conti, Khan, Bennett, Ganin, Symes, Planetary and Space Science 180:104748
    (doi 10.1016/j.pss.2019.104748). Not read; see "Could not read".
  - The 96% came through Azami et al. 2024 (arXiv:2408.05823). Quote (pdftotext): "The research revealed the potential for achieving near-complete (96%) oxygen recovery from regolith through molten salt electrolysis at approximately 950°C."
  - ESA's 17 January 2020 release, page reader: "molten calcium chloride salt to serve as an electrolyte, heated to 950°C"
  - ESA also says lunar regolith is "40–45% percent oxygen by weight".
  - The alloy list is from the authors' 236th ECS (October 2019) abstract, page reader.
- Source date: paper online 19 September 2019 (journal 2020).
- Confidence: medium. The 96% is secondary; the press reported 96% in 50 h and 75% in 15 h, which I
  did not verify.
- Use for the ship: a lower-temperature route that leaves a metal-alloy powder instead of a glassy
  slag. No kWh per kg was found.

**A24. Carbothermal reduction, measured O2 per kWh**
- Numbers:
  - 2010 field demonstration (Gustafson et al., ORBITEC): 1.45 g of O2 per kWh delivered to the
    reactor, which is about 690 kWh/kg DERIVED
  - NASA CaRD 2023: ambient 13.42 g/kWh, vacuum 11.53, 15.79 and 10.77 g/kWh, which is about 63-93
    kWh/kg DERIVED
  - process above 1600 °C with methane: MOx + x CH4 gives M + x CO + 2x H2
- Source: Azami et al. 2024, arXiv:2408.05823 (secondary), citing Gustafson et al. 2010 and Paz,
  Space Resources Roundtable 2023.
- Quote (pdftotext): "The 2010 field demonstration [62, 64] achieved an oxygen extraction rate of 1.45 grams per kilowatt-hour (g/KWh) delivered to the reactor."
- Confidence: low to medium (secondary; I could not open the NTRS full texts).
- Use for the ship: carbothermal sits about level with MRE per kg of O2 (about 60-100 kWh/kg), and it
  consumes carbon (CI asteroids carry some, A14).

**A25. Plant-scale comparison (South Pole lunar ISRU)**
- Numbers: MRE to ferrosilicon needs 6,776 kg of equipment and 311.34 kW of solar power for 25 t of
  metal plus 23.9 t of O2 a year. Mass payback 0.14 kg of hardware per kg of product. FFC is less
  efficient unless its salt use falls at least 5 times. DERIVED: about 114 kWh per kg of O2, or about
  56 kWh per kg of total product.
- Source: Azami et al. 2024 (secondary), citing Guerrero-Gonzalez and Zabel.
- Quote (pdftotext): "It requires 6776 kg of equipment and 311.34 kW of solar power to produce 25 tons of metal and 23.9 tons of oxygen annually"
- Confidence: medium (secondary).
- Use for the ship: electrolysing slag yields metal (Fe-Si) and oxygen in roughly equal mass.

**A26. DERIVED: PGM left in chondrite metal (where precious metals go)**
- Numbers:
  - Bulk precious metals (Ru+Rh+Pd+Os+Ir+Pt+Au) from A1: H 5.56 ppm, L 3.85, LL 2.94.
  - Divided by the metal fractions of A4: about 33 ppm in H metal, about 46 in L, about 72 in LL.
  - This matches Kargel's 50-220 ppm for LL metal (A9).
  - Pt alone in metal: H about 9, L about 13, LL about 22 ppm.
- Source: arithmetic on A1 and A4. It assumes PGM sit entirely in the metal (they are siderophile;
  that assumption is not sourced here).
- Confidence: medium.
- Use for the ship: precious metals follow the metal, not the slag. A well-separated slag carries
  very little PGM, and the PGM value is in the iron-nickel product, which is richer the less metal
  the rock has.

---

## 5. USGS: how Earth uses its slag

**A27. US ferrous slag sales by use, 2017 (percent of tons sold)**
- Numbers:
  - Total sales 16.0 Mt worth $398 M. Air-cooled blast-furnace 5.2 Mt, granulated 3.1 Mt, steel
    furnace 7.7 Mt.
  - Air-cooled blast-furnace slag: road bases and surfaces 47.6%, ready-mixed concrete 21.4,
    asphaltic concrete 13.2, misc. 9.2, fill 3.5, other 3.5, concrete products 1.7.
  - Granulated slag: 99.8% cementitious material.
  - Steel furnace slag: road bases and surfaces 40.8%, other or unspecified (including returns to
    furnace) 24.7, asphaltic concrete 15.1, fill 14.8, clinker raw material 2.4, misc. 2.0, concrete
    products 0.4.
  - Misc. = railroad ballast, roofing, mineral wool, soil conditioner.
- Source: H. G. van Oss, USGS 2017 Minerals Yearbook, "Slag--Iron and Steel" (advance release),
  https://d9-wret.s3.us-west-2.amazonaws.com/assets/palladium/production/atoms/files/myb1-2017-fesla.pdf
- Source date: May 2020.
- Quote (pdftotext): "About 80% of total slag sales in 2017 were air-cooled blast furnace slag and steel slag. Both slag types are mainly used as construction aggregates." The percentages are table only (Table 3).
- Confidence: high, though it is the 2017 data year (the newest edition I could read).
- Use for the ship: on Earth slag is overwhelmingly aggregate (roads, concrete, fill), plus cement for
  the glassy granulated kind. Aboard, the analogues are fill, ballast and shielding blocks. Glassy,
  quenched slag is the reactive, "cement-like" grade.

**A28. USGS Mineral Commodity Summaries 2018 (data year 2017)**
- Numbers:
  - sales 15.0 Mt (2017e); 2013-2016: 15.5, 16.6, 17.7, 16.0 Mt
  - average price $23.00/t
  - blast-furnace slag about 50% of tonnage
  - GGBFS up to about $104/t
  - world: iron slag 300-360 Mt, steel slag 170-250 Mt (2017)
- Source: USGS, Mineral Commodity Summaries, January 2018, "Iron and Steel Slag",
  https://d9-wret.s3.us-west-2.amazonaws.com/assets/palladium/production/mineral-pubs/iron-steel-slag/mcs-2018-fesla.pdf
- Source date: January 2018.
- Quote (pdftotext): "Air-cooled iron slag and steel slag are used primarily as aggregates in concrete (air-cooled iron slag only), asphaltic paving, fill, and road bases"
- Confidence: high.
- Use for the ship: the tonnage scale and the uses in words.

**A29. USGS MCS 2026 (not read; search-engine summary only)**
- Numbers (unverified): 2025 US slag sales about 16 Mt; world iron slag 330-390 Mt, steel slag
  190-290 Mt (2025).
- Source: https://pubs.usgs.gov/periodicals/mcs2026/mcs2026-iron-steel-slag.pdf returned HTTP 403 to
  WebFetch (the 2025 edition too), so these come from the search tool's summary, not the document.
- Confidence: low. Read the PDF by hand before use.
- Use for the ship: none until verified.

---

## Could not read
- https://pubs.usgs.gov/periodicals/mcs2026/mcs2026-iron-steel-slag.pdf and the mcs2025 edition:
  HTTP 403. I hoped for the latest US tonnage and uses; I used the 2017 Yearbook and the 2018 MCS
  instead.
- Jarosewich 1990, Meteoritics 25:323. ADS/SciX is a script-only page; the Smithsonian
  research.si.edu robots.txt disallows ClaudeBot and anthropic-ai; repository.arizona.edu returned
  403. I hoped for mean H/L/LL oxides with Fe metal and FeS, and CI/CM H2O and C.
- Lodders 2021 on link.springer.com: robots.txt disallows ClaudeBot and anthropic-ai. I read the NSF
  accepted manuscript instead.
- Wasson and Kallemeyn 1988 (royalsocietypublishing.org): robots.txt returned 403. Hoped for
  chondrite group means.
- Lomax et al. 2020 (eprints.gla.ac.uk disallows ClaudeBot; sciencedirect.com returned 403). Hoped
  for the 96% figure from the paper itself, the time to reach it, and energy.
- Cannon, Gialich and Acain 2023, "Precious and structural metals on asteroids", PSS 225:105608
  (mining.com 403, sciencedirect 403). Hoped for PGM grades by meteorite type.
- NASA SP-509 vol. 3, Lewis "Asteroid Resources", Tables 15 and 16 (images only in the HTML; the
  full PDF on nss.org is over 10 MB, the WebFetch limit). Hoped for metal-phase element concentrations
  in chondrites.
- core.ac.uk/download/4419447.pdf (403): a carbonyl-extraction abstract said to state a Co-rich
  residue with about 0.5% PGM. Unverified, so not used.
- mw-1.itrcweb.org glossary (403): slag and tailings definitions; EPA used instead.
- NIPR repository (503); kiss.caltech.edu (certificate error), so the Keck 2012 report was not read.
- Lewis 1993 "Resources of Near-Earth Space" and Lewis 1997: books, not online.

## Open questions
1. Free-metal weight percent for H/L/LL straight from Jarosewich 1990 Table 6. A4 is a
   density-based conversion from vol%.
2. The split of an iron meteorite's 1-1.5% non-metal (troilite, schreibersite, silicate inclusions in
   IAB/IIE irons). It sets the M-type slag recipe.
3. Liquidus of the flux-free MgO-SiO2 asteroid slag (A17), to set smelter temperature. Schreiner
   notes the melt's solidification temperature rises as electrolysis proceeds; no number was found
   for chondrite slag.
4. MRE energy for an iron-depleted (post-smelting) slag rather than raw regolith. The higher current
   efficiency (A21) suggests fewer kWh per kg of O2, but there is no measured figure.
5. Whether chondritic organic carbon is usable as an internal reductant (A14 arithmetic only).
6. PGM partition between metal and slag in a real smelt (A26 assumes all in the metal), and a primary
   source for the carbonyl-residue PGM figure.
7. The 2025 USGS tonnage and use breakdown (A29), read from the PDF by hand.
