# Can a magnetic (active) shield replace or thin the mothership drum's water shield? No, not usefully

**Research date: 2026-11-13.** Every source below was read on 2026-11-13. This is a reading of
public sources for a game and a real-life plan. **It is not engineering advice**, and nobody who
wrote it is a radiation physicist or a magnet engineer. The full working, the computed drum-scale
magnet and a chart of the cited points are in
[the active-shielding research](../research/2026-11-13-active-shielding/README.md); its
[sources.md](../research/2026-11-13-active-shielding/sources.md) holds every quotation in full.

## The question

Can superconducting magnets (or electric or plasma fields) replace, or substantially thin, the
decided passive shield of about 10 t/m2 of water around Drum 1 (5 mSv a year inside, master plan
1.5 item 8)?

Asked by the operator on 2026-11-13: "Is there a better way to do it? What about magnetic shielding?
Like generating a magnetic field to protect residents?"

## Short answer

**No.** A magnet can remove the solar-storm hazard completely, but a 10 m water shell already does.
Against galactic cosmic rays every study that models the magnet's real structure finds it removes
about a fifth to two thirds of the dose, never the roughly 99% the drum needs, and its benefit
shrinks as the passive shield thickens. Even Earth's own field would save only about 2 t/m2 of the
water at the 5 mSv level (estimate from OLTARIS tables). A drum-sized magnet would store terajoules
(kilotons of TNT), need cryogenics over 3 km2 and could weigh as much as the water it saves. The
better saving is passive: the ship's own floor, soil and stores already worth about 2 to 3 t/m2 of
water (estimate, to be checked with a transport code). Magnets remain sensible for small craft.

## Findings

1. **Real structure erases most of the gain.** Vuolo et al., Life Sciences in Space Research 8
   (2016) 22 to 29 (the EU SR2S project), read at
   https://iris.unitn.it/retrieve/e3835196-29c7-72ef-e053-3705fe0ad821/vuolo2016.pdf: "In both cases
   the resulting effective dose reduction of the active shield was not significantly improved
   compared to the "no-field" case with the same mass, reaching about 20% for configuration B2." and
   "the secondary particles, produced by the interaction of cosmic rays with the active shielding
   materials, are the major source of the crew radiation dose."
2. **Magnets alone cannot reach large cuts.** Musenich et al., IEEE Transactions on Applied
   Superconductivity 28(3), 2018, abstract at https://iris.polito.it/handle/11583/2699815: magnetic
   shielding "has several basic limitations, which restrict the reduction of the radiation dose:"
   from particles "higher than the shield cutoff energy" and secondaries; and "Despite such
   limitations, a superconducting magnet could completely eliminate the risk due to SPE."
   Hesse-Withbroe and Arquilla, arXiv:2509.09051 (10 September 2025, a preprint): "significant
   reductions in astronaut GCR doses relative to free space by an order of magnitude or more are
   unlikely to be achieved using magnetic shields alone."
3. **NASA's own concept study agrees.** MAARSS, NIAC Phase I final report, 12 November 2012,
   https://ntrs.nasa.gov/api/citations/20130000761/downloads/20130000761.pdf: "Real GCR reduction
   has been achieved however, the reduction is not significant enough to provide significantly
   greater mission duration so more work is needed."
4. **The 1975 settlement study rejected it for the same reasons.** NASA SP-413 (1977), chapter 4,
   https://settlement.nss.org/75SummerStudy/Chapt4.html: a cutoff of "10 or 15 GeV/nucleon" would
   reach 0.5 rem a year, but "the structural mass required to resist the magnetic forces between
   superconducting coils" "precludes this design even for the most favorable geometry, namely, a
   torus." Electric shielding "seems infeasible since a 10-billion-volt potential" "would be required
   for even moderate shielding."
5. **A field's help shrinks with depth.** Globus and Strout, NSS Space Settlement Journal, April
   2017 (OLTARIS), https://nss.org/wp-content/uploads/2018/01/NSS-JOURNAL-Orbital-Space-Settlement-Radiation-Shielding.pdf,
   Tables 2 and 3: behind polyethylene in an equatorial orbit at 600 km (under Earth's field) the
   dose is 14.0 mSv a year at 1 t/m2 and 7.0 at 5 t/m2, against 194 and 35 in deep space, so the
   field leaves 7% of the dose at 1 t/m2 and 20% at 5 t/m2 (computed). The paper: "These high energy
   particles initiate nuclear interactions in the shielding that produce secondary particles,
   leading to an increase in exposure."
6. **People's limit on the field.** ICNIRP, Health Physics 96(4), 2009,
   https://www.icnirp.org/cms/upload/publications/ICNIRPstatgdl.pdf: "acute exposure of the general
   public should not exceed 400 mT (any part of the body)", and for implants "much lower restriction
   levels, such as 0.5 mT (IEC 2002)."

## What is still unknown, and what would settle it

- **Doses past 7 t/m2 of water** are an extension of one table; one OLTARIS run of the drum's real
  layered stack (water, gap, steel, soil, air, the far floor) would settle both this and the floor's
  credit. Cost: a free NASA account (the operator's decision) and an afternoon.
- **A drum-scale magnet's mass and cryogenic power** are scaled from magnets a hundredth its size;
  only a dedicated magnet design study would settle them, and the conclusion above does not depend
  on them (the dose physics alone rules a magnet out as a replacement).

## Sources you could not reach

The SR2S and Musenich papers' full text in IEEE Transactions on Applied Superconductivity
(paywalled); Washburn et al. 2015 in Life Sciences in Space Research (no open copy found); the
Hoffman et al. 2005 NIAC report (not found on NTRS); Musenich's 2017 CERN Indico slides (robots.txt
forbids contribution PDFs). Details in the research's sources.md.

## What it means for this project

**Certain (from the sources):** magnets remove the solar-storm hazard; against cosmic rays they cut
the dose by tens of percent, not by the factor of about 100 the drum needs; the magnet's own
material makes neutrons a field cannot bend; a field adds less the thicker the passive shield.

**Judgement calls (ours):** keep the passive water shell for the drum and add no magnet; credit the
ship's own floor and stores once a transport run confirms it (likely 7 to 8 t/m2 of water instead
of 10); keep active shields for small craft in the game, where their mass pays. Not engineering
advice.
