# Sources for the active-shielding research (helper file S)

**Read 2026-11-13** unless an entry says otherwise. Each entry: what it is, its own date, what it
gives this research, the words that carry the meaning (quoted exactly), and how far to trust it.
The README cites these as S1, S2 and so on; the sizes research's own entries (F28 to F36) are in
[its helper file F](../2026-11-13-mothership-sizes/F-structure-and-shielding.md).

Web reading followed the wave rules: robots.txt read first for every host below (cordis.europa.eu,
frontiersin.org, iris.unitn.it, iris.polito.it, arxiv.org, nss.org, spenvis.oma.be, iter.org,
science.nasa.gov; icnirp.org has no robots.txt; indico.cern.ch forbids contribution PDFs, so none
was opened), a plain tool user agent, and no download beyond the pages and the PDFs of the sources
cited here.

## Passive shielding

### S1. Globus and Strout, "Orbital Space Settlement Radiation Shielding"
- What: NSS Space Settlement Journal, April 2017, every number computed with NASA's OLTARIS tool.
  https://nss.org/wp-content/uploads/2018/01/NSS-JOURNAL-Orbital-Space-Settlement-Radiation-Shielding.pdf
  (the sizes research read its Table 2 on 2026-11-12; this pass re-read Tables 2, 3 and 4).
- Gives: Table 2, yearly dose in deep space behind polyethylene, water and lunar regolith from 0
  to 7 t/m2 (regolith to 11); Table 3, polyethylene in equatorial orbits at 600 to 1,000 km, 0 to 5
  t/m2; Table 4, polyethylene at 500 and 600 km, 0.01 to 2 t/m2. The chart's points are these
  tables' values (shield-curves.json).
- Quotations: "Note that polyethylene is a bit more effective than water, and both are quite a bit
  more effective than lunar regolith." / "All of the data presented here were calculated for a low
  solar activity period from 17 June 1977 to 17 June 1978 and are thus conservative." / "There is
  about a factor of three difference in the space radiation between solar minima and maxima" (the
  paper cites Robbins 1996) / "Table 4
  suggests that for settlements in low equatorial orbit (below 500 km), no shielding mass is
  required to meet the 20 mSv/yr" / on the orbit cases: "These high energy particles initiate
  nuclear interactions in the shielding that produce secondary particles, leading to an increase in
  exposure."
- Trust: medium. NASA's code, an advocacy society's journal, effective dose for one body model.
  Table 3 gives 14.0 mSv a year at 600 km behind 1 t/m2 and Table 4 gives 14.7 for the same case:
  the paper's own small disagreement, left as printed.

### S2. Cucinotta, Kim and Ren, "Evaluating Shielding Effectiveness for Reducing Space Radiation Cancer Risks"
- What: NASA Johnson Space Center paper, NTRS 20070005030 (the PDF's file date is 10 August 2005).
  https://ntrs.nasa.gov/api/citations/20070005030/downloads/20070005030.pdf
- Gives: the per-kilogram order of hydrogen, polyethylene and aluminium at one depth. Table 5a,
  solar minimum, 20 g/cm2 shields, a Mars swing-by mission, effective dose E in Sv.
- Quotations (Table 5a rows): "Aluminum  0.87", "Polyethylene 0.78", "Hydrogen  0.43".
- Trust: high for the order (NASA's risk model); one thickness only, so it is not on the chart.

### S3. MAARSS, NIAC Phase I final report (also used under active shielding, S7)
- Section 7 (Singleterry, NASA Langley) on liquid hydrogen: "The long held belief that liquid
  hydrogen is the best shielding material needs to be qualified. For missions that last longer than
  225 days, the same mass of polyethylene or water allow more days for a mission than liquid
  hydrogen." It gives liquid hydrogen's density as 0.07099 g/cm3 and ran OLTARIS "at numerous
  sphere thicknesses from 0.0001 to 1000 gm/cm2" in aluminium, liquid hydrogen, polyethylene and
  water, but shows the results only as a figure of dose against total mass, so no point from it is
  on the chart.

## The Earth's field

### S4. Baldini, "Space-Based Cosmic-Ray and Gamma-Ray Detectors: a Review"
- What: arXiv:1407.7631, 29 July 2014 (revised 4 August 2014), lecture notes for the 2014 ISAPP
  school. https://arxiv.org/abs/1407.7631
- Gives: why the poles are open and the equator shielded, and the cutoff numbers.
- Quotations: "The field being strongest at the magnetic poles, one might naively think that the
  shield effect is stronger there than at the equator. In fact is quite the opposite, as the field
  at the poles is radial and charged particles coming from the zenith direction can travel
  unaffected along the field lines. At the equator, on the other hand, the field is orthogonal to
  the zenith direction and the shielding effect is maximum." / "Near the equator the typical cutoff
  value is of the order of 15 GV, while sub-GV values can be reached moving close to the magnetic
  poles." (The PDF's text renders "naively" with a diaeresis and puts a "~" before 15; both shown
  plainly here.)
- Trust: high for textbook physics; lecture notes, not a primary measurement.

### S5. NASA Science, "Earth's Magnetosphere: Protecting Our Planet from Harmful Space Energy"
- What: NASA web page, 3 August 2021, updated 23 October 2024.
  https://science.nasa.gov/science-research/earth-science/earths-magnetosphere-protecting-our-planet-from-harmful-space-energy/
- Quotations: "trapping most of it a safe distance from Earth's surface in twin doughnut-shaped
  zones called the Van Allen Belts." / "continually bouncing back and forth from pole to pole."
- Trust: high (NASA's public explanation).

## Active shielding

### S6. Vuolo, Giraudo, Musenich, Calvelli, Ambroglini, Burger and Battiston, "Monte Carlo simulations for the space radiation superconducting shield project (SR2S)"
- What: Life Sciences in Space Research 8 (2016) 22 to 29, peer reviewed (received 7 May 2015,
  accepted 30 December 2015). The authors' repository copy:
  https://iris.unitn.it/retrieve/e3835196-29c7-72ef-e053-3705fe0ad821/vuolo2016.pdf
- Gives: the SR2S toroid modelled with its real structure. Table 1: a 10 m long toroid, inner
  radius 2.7 to 2.8 m, outer 6.3 to 8.75 m, bending power 7.9 or 11.9 T m, total masses 315, 104
  and 147 t; conductor field at most 4 T. Table 2: effective dose as a share of the free-space dose,
  field off and on (the chart's SR2S points are field on divided by field off).
- Quotations: "The toroidal field has in fact many advantages, including an isotropic protection
  around the habitat and a very low fringe field inside the internal module." / "Clearly a toroid
  surrounding the spacecraft leaves the endcaps of the habitat module free of the field (and
  therefore protection)" / "The magnet is supposed to operate in persistent mode at 10 K, cooled by
  cryocoolers." / "The 100% density value corresponds to about 315 tons of materials and about 100
  g/cm2 of areic mass." / "Considering configuration A, with 100% of mass, the total lateral
  effective dose is reduced by a factor of about 2. It should be recalled that this value is
  obtained considering only a fictitious lateral source" / "in a realistic situation the secondary
  particles, produced by the interaction of cosmic rays with the active shielding materials, are
  the major source of the crew radiation dose. Moreover a large part of the secondary particles are
  fast neutrons that cannot be deflected by the magnetic field" / "In both cases the resulting
  effective dose reduction of the active shield was not significantly improved compared to the
  "no-field" case with the same mass, reaching about 20% for configuration B2."
- Trust: high (peer reviewed, Geant4 with the structure modelled); the source was lateral only, so
  the open ends are not counted.

### S7. Westover et al., "MAARSS: Magnet Architectures and Active Radiation Shielding Study", NIAC Phase I final report
- What: NASA Johnson Space Center with the Advanced Magnet Lab, INFN, Florida State, Colorado and
  NASA Langley, 12 November 2012. NTRS 20130000761.
  https://ntrs.nasa.gov/api/citations/20130000761/downloads/20130000761.pdf
- Gives: a 6+1 solenoid array with a compensator coil for a 6 m x 10 m habitat; doses at 8 T m;
  the SPE comparison with polyethylene; quench physics; mass and power (Table 9.1).
- Quotations: "Most toroid configurations result in a large field within the habitat requiring
  complex shielding from the magnetic field for the crew and electronics." / "As a protection from
  SPE, analysis was completed for a 6m diameter by 10m length cylindrical habitat that resulted in
  141 metric tons of 75 cm thick polyethylene providing similar protection as that from a 6+1
  expanding coil array at 8 Tm with a simulated mass of 36 metric tons." / "Dose equivalent results
  for the 8 Tm configuration are comparable with annual dose equivalents for a spacecraft in LEO
  with "barrel-only" BFO results at 309 mSv and total results with nearly "naked" end caps with an
  empty habitable volume at 451 mSv." / "Real GCR reduction has been achieved however, the
  reduction is not significant enough to provide significantly greater mission duration so more
  work is needed." / "The increase in the field density from 1 to 2.5 T results in a modest
  reduction of the dose levels (25%) with respect to the estimated factor 2 increase required for
  the shield mass." / "each base solenoid of the radiation shield with a volume of about 1000 m3 and
  a flux density of 1 Tesla stores about 400 MJ." / "the energy stored in a single base coil of the
  radiation shield is sufficient to melt about 570 kg of copper." / "An uncontrolled release of
  this energy into the coil itself would cause significant damage to the magnet with high
  probability." / "energy to initiate a quench in an HTS superconducting coil operating above 30 K
  is about 4 orders of magnitude larger than for an LTS coil operating at 4.2 K." / "the quench
  propagation velocity (about 2 cm/sec for HTS and typically several m/sec in LTS conductors)" /
  Table 9.1: coils "~330 W/coil (~ 2 kW)", "Compensator coil 2126" kg at "26 kW". / Section 7: "The
  variation of the dose over the solar cycle, an 11 year cycle, is on the order of a factor of 2"
- Trust: medium to high (a NASA-led concept study, not peer reviewed; Monte Carlo and HZETRN).

### S8. Hesse-Withbroe and Arquilla, "An Improved Rapid Performance Analysis Model for Solenoidal Magnetic Radiation Shields"
- What: arXiv:2509.09051, 10 September 2025. A preprint, not yet peer reviewed.
  https://arxiv.org/abs/2509.09051
- Gives: the most recent trade study; doses for 15 and 100 T m solenoids behind a 12 g/cm2
  aluminium wall (Table 1: 1977 solar minimum, habitat 3 m radius, 10 m tall).
- Quotations: "For a 20 m tall, 3 T × 5 m shield, the inner and inner transition surfaces
  respectively contribute 44.6 and 20.6 mSv/yr to the total dose of 246 mSv/yr." / "the new model
  predicts a dose equivalent of 188 mSv/yr, a reduction of only 63%." (for the "20 m tall, 10 T ×
  10 m shield") / "Relative dose reduction is defined with respect to the GCR dose received at a
  point protected only by the habitat pressure vessel (514 mSv/yr)." / "greatly diminished
  performance for strong shields with bending powers greater than 20 T-m" / "magnetic shields
  enable significant mass savings over passive shields for mission scenarios where the requisite
  dose reduction is greater than about 60% relative to free space" / "significant reductions in
  astronaut GCR doses relative to free space by an order of magnitude or more are unlikely to be
  achieved using magnetic shields alone."
- Trust: medium (a preprint; it agrees with S9 and S6).

### S9. Musenich, Calvelli, Giraudo, Vuolo, Ambroglini and Battiston, "The Limits of Space Radiation Magnetic Shielding: An Updated Analysis"
- What: IEEE Transactions on Applied Superconductivity 28(3), 2018, DOI 10.1109/TASC.2017.2785805.
  Abstract read on the Politecnico di Torino repository page:
  https://iris.polito.it/handle/11583/2699815 (the paper itself is behind IEEE's paywall).
- Quotations: "However, magnetic shielding has several basic limitations, which restrict the
  reduction of the radiation dose:" "they range from the biological effect of the particles in the
  high region of the GCR spectrum," "higher than the shield cutoff energy, to the generation of
  secondary particles" "due to the interaction of cosmic rays with magnet and spacecraft
  materials." / "Despite such limitations, a superconducting magnet could completely eliminate the
  risk due to SPE." (S8 summarises the paper: magnetic shields "are not likely to be viable as a
  sole countermeasure if GCR dose reductions on the order of 90% relative to free space are
  desired.")
- Trust: high for the abstract (peer reviewed); the body not read.

### S10. SR2S final report summary (CORDIS)
- What: the European Commission's record of FP7 project 313224, run 1 January 2013 to 31 December
  2015; page last updated 3 June 2016. https://cordis.europa.eu/project/id/313224/reporting
- Quotations: "it could provide a factor two in reduction of the CR dose with suitable technical
  developments;" / "The mass of the whole system composed by four toroidal magnets and the main
  structure is around 50 tons." / "The SR2S magnet is based on a titanium clad MgB2 wire that
  guarantees lightness, reliability and simple cryogenics." / "with the maximum flux around 1 GeV
  for protons and 500 to 600 MeV/nucleons for ions."
- Trust: medium (a project summary written by the project). The page reader returned these as
  pieces; they were not checked against a second copy.

### S11. NASA SP-413, "Space Settlements: A Design Study" (1977), chapter 4
- What: the 1975 Stanford and NASA Ames summer study, hosted by the National Space Society.
  https://settlement.nss.org/75SummerStudy/Chapt4.html (nss.org redirects there).
- Quotations: "An ideal shield would bring the radiation dosage below 0.5 rem/yr cheaply" / "As a
  consequence a magnetic field around the torus with a cutoff of 0.5 GeV/nucleon and a structural
  mass of about 10 kt," "corresponding to a thickness of matter of 0.5 t/m^2, would actually
  increase the exposure to about 20 rem/yr." / "A cutoff of 10 or 15 GeV/nucleon would eliminate so
  many of the high energy particles" "that even with secondary production the dose would not be
  above 0.5 rem/yr." / "The difficulty is that the structural mass required to resist the magnetic
  forces between superconducting coils" "precludes this design even for the most favorable
  geometry, namely, a torus." / "Similarly, electric shielding by a static charge seems infeasible
  since a 10-billion-volt potential" "would be required for even moderate shielding."
- Trust: medium (1975 physics and costs; the page reader returned the sentences in pieces).

### S12. Bamford et al., "An exploration of the effectiveness of artificial mini-magnetospheres as a potential Solar Storm shelter for long term human space missions"
- What: Acta Astronautica 105 (2014) 385 to 394; arXiv:1406.1159v2, 5 June 2014.
  https://arxiv.org/abs/1406.1159
- Gives: the plasma-filled "mini-magnetosphere", meant for solar storms.
- Quotations: "The maximum feasible coil radius, a of 3.0m (set by the launch rocket cowling I/D)"
  ... "This produces a peak magnetic field, Bo, of" 6.4 T / "16kW, and a 5kW for the cryoplant and
  the control system." with a total mass of 1.5 x 10^3 kg (the PDF's symbols garble in extraction:
  the numbers are given plainly) / "An active deflector shield system would never be a replacement
  for passive shielding nor biological advances."
- Trust: medium (peer reviewed, but the plasma effect is argued from theory, laboratory work and
  lunar swirls, not demonstrated around a craft).

### S13. Tripathi, "Meeting the Grand Challenge of Protecting Astronaut's Health: Electrostatic Active Space Radiation Shielding for Deep Space Missions", NIAC 2011 final report
- What: NASA Langley, NTRS 20160010094.
  https://ntrs.nasa.gov/api/citations/20160010094/downloads/20160010094.pdf
- Quotations: "The 10-sphere design requires potentials of 300 MV, and would be used in conjunction
  with passive material shielding. The author notes that the ability to achieve 300 MV potential
  levels remains as future work." / "The EIMS power requirement becomes increasingly more
  challenging as the spacecraft voltage is increased." / experiments "with up to 10kV of membrane
  charging".
- Trust: medium (a concept study).

## Magnets on Earth, and the limits for people

### S14. Védrine, "Large Superconducting Magnet Systems"
- What: CEA Saclay, CERN Accelerator School proceedings; arXiv:1501.07169 (2015).
  https://arxiv.org/abs/1501.07169
- Quotations: "If no precautions have been taken, when a magnet quenches the entire stored energy
  will be dissipated in the area of the quench initiation point." / "Quench protection is the major
  requirement in the design of detector magnets, because the stored energy can be as high as 1-3
  GJ." (the PDF has an en dash in "1-3") / CMS: "a nominal magnetic field of 4 T at the operating
  current of 19.5 kA, in a 5.9 m diameter and 12.5 m long warm bore, leading to a stored energy of
  2.7 GJ." / Table 6, the ATLAS barrel toroid: inner bore 9.4 m, outer diameter 20.1 m, length 25.3
  m, 8 coils, stored energy 1,080 MJ, peak field 3.9 T, total weight 830 t; "ATLAS is now the
  largest superconducting magnet system in the world."
- Trust: high (a CERN school text by a magnet builder).

### S15. ITER, "Magnets"
- What: ITER Organization web page, undated (footer "© 2026"). https://www.iter.org/machine/magnets
- Quotations: "Ten thousand tonnes of magnets, with a combined stored magnetic energy of 51
  Gigajoules (GJ)," / "The toroidal field coils are designed to produce a total magnetic energy of
  41 gigajoules" / "a maximum magnetic field of 11.8 tesla"
- Trust: high (the builder's page; design values).

### S16. ICNIRP, "Guidelines on Limits of Exposure to Static Magnetic Fields"
- What: Health Physics 96(4):504 to 514, 2009. https://www.icnirp.org/cms/upload/publications/ICNIRPstatgdl.pdf
- Quotations: "It is recommended that occupational exposure of the head and trunk should not exceed
  a spatial peak magnetic flux density of 2 T" / "exposure up to 8 T can be permitted if the
  environment is controlled" / "acute exposure of the general public should not exceed 400 mT (any
  part of the body)." / "these considerations can lead to much lower restriction levels, such as
  0.5 mT (IEC 2002)." (for implanted electronic medical devices, ferromagnetic implants and flying
  objects)
- Trust: high (the international guideline).

## Could not read, or did not open

- The SR2S papers in IEEE Transactions on Applied Superconductivity (Battiston et al. 2013, Musenich
  et al. 2014, Calvelli et al. 2017) and Musenich et al. 2018's body: paywalled; abstracts or the
  repository record only.
- Washburn, Blattnig, Singleterry and Westover, "Active magnetic radiation shielding system analysis
  and key technologies", Life Sciences in Space Research 4 (2015) 22 to 34: no open copy found
  (only library catalogue records); S8 extends its model.
- Hoffman, Fisher and Batishchev, NIAC Phase I 2005, "Use of superconducting magnet technology for
  astronaut radiation protection": not found on NTRS in this pass.
- The CERN Indico contribution PDF of Musenich's 2017 talk: robots.txt disallows contribution PDFs,
  so it was not opened.
- ECSS-E-ST-10-04C on SPENVIS (Størmer theory): read, but the formula is an image; S4 used instead.
- Wilson et al., "Shielding Strategies for Human Space Exploration", NASA CP 3360 (1997), which may
  hold thick-shield tables for liquid hydrogen and aluminium: not opened (the copy found was a
  third-party mirror; NTRS 19980137598 not tried for lack of time). The next thing to read for the
  chart's missing series.
- NASA/TP-2005-213164 (Cucinotta et al., Managing Lunar and Mars Mission Radiation Risks, Part I):
  NTRS shows "No Preview Available".
