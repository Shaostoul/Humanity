# How much light each home crop needs: daily light integrals

**Research date: 2026-09-26.** Every source below was read on 2026-09-26. This is
a reading of public extension and research sources, done to give the game's garden
honest numbers. **It is not agronomic advice.** Nobody who wrote it is an
agronomist; before sizing real lamps for a real crop, check a local extension
service.

**The question.** For each crop the HumanityOS home grows in its towers, beds,
trays and fields, what daily light integral (DLI: moles of photosynthetic photons,
400 to 700 nm, per square metre per day) is the least it still grows acceptably
at, what is the recommended level, and above what level does more light give
little or no more growth?

Prepared for the garden light model (`src/systems/farming/lighting.rs`, numbers
in `data/garden/lighting.ron`), which on this date plans every crop at 17
mol·m-2·d-1, the Cornell figure for one Boston bibb lettuce cultivar.

The crop list is every `plant:` value in `data/towers/aeroponic_configs.ron`, the
`bed_crops` values in `data/world/showcase.ron`, the `default_crop` values in
`data/garden/grow_media.ron`, plus the field crops named in the brief. That is 63
ids, all confirmed present in the `id` column of `data/plants.csv`.

---

## Short answer

- **Sourced figures were found for 27 of the 63 crops.** 35 have no crop-specific
  figure in anything read, and are listed under "Not found". Oyster mushroom is a
  fungus and does not photosynthesise, so a DLI does not apply to it.
- **Leafy greens and most kitchen herbs sit near today's 17.** Lettuce 12 / 17 /
  17, where the 17 ceiling is a tipburn limit that holds only with air blown down
  onto the plants (Cornell could not exceed 12 without it). Spinach 14 / 17 / 20.
  Parsley, mint and sage peak at about 15 to 16 and then lose weight. Radish peaks
  at 12 to 15 and cracks more above 15.
- **Fruiting crops want about half as much again.** Tomato, cucumber and zucchini
  have recommended ranges of 20 to 30 (targets used: 25). Bell pepper a little
  less (22). Strawberry 20 to 25, with stress reported above 30.
- **Several crops had not stopped responding at the top of the range tested.**
  Basil, cilantro, dill, oregano, thyme (all still rising at 20) and kale (still
  rising at 22). Their true saturation is higher and unknown.
- **Grains, soybean and potato under lamps were grown at 35 to 67** in the studies
  found, two to four times the lettuce figure, and wheat still had not saturated
  at 150 (with CO2 enriched to 1200 ppm). This is the quantitative reason cereals
  are the worst indoor crop per unit of electricity.
- **Many values are the ends of recommended ranges, not measured thresholds, and
  several come from CO2-enriched chambers.** Every value in the table at the end
  carries a flag saying which.

---

## How the three numbers were chosen

- **dli_min**: a minimum the source states outright; otherwise the low end of a
  source's recommended range (flag `RL`); or, where nothing better exists, the
  lowest level a study tested at which the crop still grew (flag `LT`).
- **dli_target**: a level the source recommends outright; otherwise the midpoint of
  a recommended range (flag `MID`); for a crop still gaining at the top of a
  study's range, the highest level tested (flag `NR`).
- **dli_saturation**: a measured plateau or a harm threshold; otherwise the high end
  of a recommended range (flag `RH`). `NR` means growth was still increasing at this
  level, so the true saturation is higher: treat it as a floor, not a ceiling.
- Other flags: `CO2` measured with CO2 enriched to 1000 to 1500 ppm (ambient-air
  values are probably lower); `GRP` taken from a figure the source gives a named
  group that includes this crop; `ORN` from an ornamental finished-plant quality
  chart, not a harvest study; `SB` a speed-breeding protocol level, sized for fast
  seed-to-seed cycling, not for maximum yield; `SEC` a secondary source whose
  primary was not read.
- Units are mol·m-2·d-1 of photosynthetic photons throughout. Where a source gives an
  instantaneous flux (PPFD, µmol·m-2·s-1) and a photoperiod, DLI = PPFD × hours ×
  3600 / 1,000,000, and the arithmetic is shown.
- **Quotations** reproduce the source's words and numbers exactly, including its
  typographical slips (marked [sic]). Unit glyphs (middle dots, minus signs,
  superscripts) that PDF text extraction dropped or garbled are written in plain
  text; nothing else is changed.

---

## Context figures (not crop values)

- Natural light. Ohio State (S8): "Under natural light, DLI can exceed 60 mol/m2/d
  during summer long day conditions and can be less than 5 mol/m2/d during overcast
  winter short day conditions." Purdue (S1): in the northern US from December to
  March "naturally occurring outdoor DLI values are between 5 to 30 mol·m-2·d-1", and
  "In a greenhouse, values seldom exceed 25 mol·m-2·d-1".
- Crop light classes, Purdue (S1, for floriculture crops): "Crops with a DLI
  requirement of 3 to 6 mol·m-2·d-1 are considered low-light crops, 6 to 12
  mol·m-2·d-1 are medium-light crops, 12 to 18 mol·m-2·d-1 are high-light crops, and
  those requiring more than 18 mol·m-2·d-1 are considered very high-light crops."
  Iowa State (S9) uses a different scale for food crops: "low (5 to 10 mol·m-2·d-1),
  medium (10 to 20 mol·m-2·d-1), high (20 to 30 mol·m-2·d-1) and very high light
  (>30 mol·m-2·d-1)". The two scales disagree on the word "high"; the table uses
  numbers only.
- The yield rule of thumb, Runkle (S4): "As a general rule, a 1 percent increase in
  DLI increases production by 1 percent." Dorais (S6) calls the same idea "the
  one-percent rule" and says it "often gives close estimates of the consequences of
  light loss on tomato yield". Below saturation, growth roughly proportional to light
  is the model the sources support.
- Continuous light. Runkle (S4): "Some crops, especially tomato, become stressed and
  develop chlorotic leaves if grown under continuous light. Therefore, four to six
  hours of darkness is suggested each night." Wheeler et al. (S20): "some cultivars
  like Kennebec, Superior and Norchip were physiologically intolerant to continuous
  light". This matters to the game because a plot lit by the sun by day and a grow
  light all night is under continuous light (see "What it means").

---

## Crop by crop

Figures are dli_min / dli_target / dli_saturation. Source keys (S1, S2, ...) point
to the list after this section.

### Leafy greens

**lettuce: 12 / 17 / 17**

- min 12: Runkle (S4): "A recommended minimum DLI for lettuce production is 12 to
  14 mol·m-2·d-1". The low end is used because two more sources put the floor at the
  same place: Dorais (S6): "Light integral of 12-13 mol m-2 d-1 or higher are
  generally needed for lettuce production", and Virginia Tech (S5): "Lettuce 12−17".
- target 17: Cornell (S2): "For the lettuce production the recommended level is 17
  mol/m2/d." Cross-check against a second source: NASA's best lettuce productivity
  was at a daily PAR of 16.8 (S17, Table 2), where "Lower PAR levels were used to
  reduce the incidence of leaf tipburn".
- saturation 17 (a harm threshold, not a growth plateau): Cornell (S2): "A 17
  mol/m2/d DLI target has to be matched with sufficient downward air flow to prevent
  tip burn. Without the air flow, we were not able to go over 12 mol/m2/d." Also:
  "For some cultivars, 15 or [sic] mol/m2/d is the maximum amount of light that can
  be used before the physiological condition called tipburn occurs." The limit is
  calcium delivery to young leaves, not photosynthesis: with air blown onto the
  growing point and CO2 at 1200 ppm, Frantz et al. (S19) took lettuce to 57.6,
  "two to three times higher than normally used for lettuce", and
  "Eliminating tipburn doubled edible yield at the highest PPF level." **For a
  still-air bed, the Cornell figure is 12.**
- photoperiod: Dorais (S6): "Lettuce is a day neutral or long-day plant".

**spinach: 14 / 17 / 20**

- min 14 (`RL`): Virginia Tech (S5): "Spinach 14−20". Montana State (S14) found
  spinach less sensitive to low light than lettuce: dry weight rose only 42% from 8
  to 14 (quotation under kale).
- target 17: Cornell baby spinach handbook (S3): "The total light integral received
  by spinach once plants are floated in the ponds should be at least 17 mol/m2/d."
- saturation 20 (`RH`): Virginia Tech's high end. Cornell's set-point table gives
  "17 - 22 mol/m2 /d combination of solar and supplemental light", but with CO2 at
  "1000-1500 ppm if light is available", and says "we did not experimentally
  optimize daily light integral" and "a higher DLI can be tolerated". The
  ambient-air figure (20) is used; 22 is the CO2-enriched alternative. Neither is a
  measured plateau.
- Note: Cornell's crop is baby-leaf spinach cut at 14 days.

**kale: 8 / 22 / 22**

- min 8 (`LT`): Baumbauer et al., Montana State (S14): "DW for all species increased
  in a linear fashion under increasing DLI, with lettuce increasing 203%, kale 47%,
  and spinach 42% as DLI increased from 8 to 14". Kale grew at 8, the lowest level
  tested; no threshold was found below which it failed.
- target 22 (`NR`): Currey and Yost, Iowa State (S13), 2 to 22 tested: "we found no
  saturation in growth responses to DLI for kale, pac choi, or Swiss chard", and
  "Even when grown at the high end of DLIs in our study (i.e. 18 to 22
  mol·m-2·d-1), kale, pac choi and Swiss chard had fluorescence values that
  indicated healthy, non-stressed plants." They class kale as a high-light (20 to
  30) or very-high-light plant.
- saturation 22 (`NR`). Same-species context, not used as the value: collards are
  also Brassica oleracea, and Gagne (S15) found "for lettuce and collards declining
  biomass benefits were found above 24 mol m-2 d-1" (baby leaf, 12-day harvests).
  The kale ceiling is probably somewhere above 22, perhaps near 24.

### Kitchen herbs

**basil: 12.9 / 20 / 25**

- min 12.9: Dou et al., Texas A&M (S12): "we suggest a DLI of 12.9 mol·m-2·d-1 for
  sweet basil commercial production in indoor vertical farming to minimize the
  energy cost while maintaining a high yield and nutritional quality." A least-cost
  level, used here as the minimum.
- target 20: Currey and Litvin, Iowa State (S9): "fresh weight increased linearly
  with DLI up to 20 mol·m-2·d-1 for basil, cilantro, dill, thyme and oregano, with no
  decrease at high DLI values." Walters et al. (S10), citing Litvin's dissertation:
  "increasing DLI from 2 to 20 mol·m-2·d-1 increased sweet basil 'Nufar' fresh mass
  linearly by 136.4 g". Virginia Tech (S5): "Basil 15−25", whose midpoint is also 20.
- saturation 25 (`RH`): Virginia Tech's high end. **The sources disagree here.** Dou
  et al. (S12) found "The shoot FW under DLIs of 12.9, 16.5, and 17.8 mol·m-2·d-1
  was 54.2%, 78.6%, and 77.9%, respectively, higher than that at a DLI of 9.3", that
  is, no gain from 16.5 to 17.8 (fluorescent lamps, a compact cultivar, 21 days from
  germination). Iowa State saw a straight-line rise to 20 (greenhouse, repeated
  across seasons). Judgement: the wider-range Iowa State work and the Virginia Tech
  range are weighted more; a reader who prefers Dou would set saturation near 16.5.

**cilantro: 15 / 20 / 20**

- min 15 (`RL`): Virginia Tech (S5): "Cilantro 15−20".
- target and saturation 20 (`NR`): Currey and Litvin (S9), quoted under basil, and:
  "basil, cilantro, dill, thyme and oregano have high or very high DLI requirements,
  as the optimal DLI was not identified with the range of DLI values in our study."

**dill: blank / 20 / 20**

- min: not found.
- target and saturation 20 (`NR`): Currey and Litvin (S9), as for cilantro. Walters
  and Lopez (S11) add that the light response depends on warmth: "For dill,
  increasing DLI decreased fresh mass when MDT was low (9.7 to 13.9 °C) and increased
  fresh mass when MDT was high (18.4 to 27.2 °C)." (MDT is mean daily temperature.)

**oregano: blank / 20 / 20**

- min: not found. Target and saturation 20 (`NR`): Currey and Litvin (S9), as above.

**thyme: 8 / 20 / 20**

- min 8 (`ORN`): Purdue HO-238 (S1), Table 2 row "Thymus", from James E. Faust's
  chart in the Ball Red Book. The yellow band ("Minimum aceptable [sic] quality")
  covers the 8 and 10 columns, green ("Good quality") 12 to 16, red ("High quality")
  18 to the chart's edge at 30. These are quality bands for finished potted plants,
  not herb harvest weights; the bands are read from the chart, which has no numbers
  in words.
- target and saturation 20 (`NR`): Currey and Litvin (S9), as above.

**parsley: 10 / 15 / 15**

- min 10 (`RL`): Virginia Tech (S5): "Parsley 10−15".
- target and saturation 15: Currey and Litvin (S9): "The fresh mass of parsley, mint
  and sage increased as DLI increased up to ~15 to 16 mol·m-2·d-1, after which
  weight started to decrease." Virginia Tech's high end (15) agrees. Walters and
  Lopez (S11), over 6.2 to 16.9: "While DLI did not affect parsley in our study
  (besides DMC)", which fits a plateau at or below that range's top.

**mint: 10 / 14.9 / 14.9**

- min 10 (`GRP`): Currey and Litvin (S9) put mint in their medium class, "medium (10
  to 20 mol·m-2·d-1)", saying "we found parsley, mint and sage are medium-light
  crops". The class's lower bound is used.
- target and saturation 14.9: Walters et al. (S10), citing Litvin: "Mint and sage
  fresh mass increased as DLI increased from 2 mol·m-2·d-1 to an optimum DLI
  (DLIopt) of 14.9 and 15.9 mol·m-2·d-1, respectively". Studied as Mentha spp.

**sage: 10 / 15.9 / 15.9**

- Same two sources as mint (S9 class lower bound; S10 optimum 15.9).

**lavender and echinacea: 8 / 18 / blank**

- `ORN`: Purdue HO-238 (S1), Table 2 rows "Lavendula (lavender)" and "Echinacea":
  minimum acceptable band 8 to 10, good 12 to 16, high quality 18 and above. Target
  is the start of the high-quality band. No saturation is given: the high-quality
  band runs to the chart's edge (30). These are finished ornamental plants, not
  flower or root harvests.

### Fruiting crops

**tomato: 15 / 25 / 30**

- min 15 (`GRP`): Runkle (S4): "at least 15 (and preferably more than 20)
  mol·m-2·d-1 is suggested for vine crops", in an article about "vine crops like
  tomato, pepper and cucumber". **Other minimums, lower and higher**, shown so a
  reader can pick differently: Purdue HO-238 (S1) chart row "Lycopersicon (tomato)"
  has its minimum-acceptable band at 10 to 12; Dorais (S6): "Light levels below 1.5
  MJ m-2 of solar radiation (3.1 mol m-2 d-1) result in an increased incidence of
  reduced fruit set and poor flower quality"; Morgan (S7): "a mature tomato crop
  needs an estimated DLI minimum of 22 or more mol m-2 d-1 for good production."
  Runkle's 15 is used because it is the extension figure stated as a production
  minimum, and it sits between the others.
- target 25 (`MID`): Virginia Tech (S5): "Tomato 20−30".
- saturation 30 (`RH`): Virginia Tech's high end. Not a measured plateau: Dorais
  (S6): "A light requirement equal or higher than 30 mol m-2 d-1 is reported for a
  tomato culture", and NASA's best tomato productivity was at 38.6 (S17, `CO2`).
- photoperiod: day-neutral (`GRP`): Runkle (S4): "Most vegetable crops are day
  neutral". Needs 4 to 6 hours of darkness a night (context section).

**pepper (bell): 12 / 22 / blank**

- min 12: Dorais (S6): "Sweet pepper needs a light integral of at least 12 mol m-2
  d-1 during the winter time for a good control of the production cycles." Purdue's
  chart row "Capsicum (pepper)" (S1) puts the minimum-acceptable band at 10 to 12.
- target 22: the same Purdue row starts its high-quality band at 22; Runkle (S4)
  says "preferably more than 20" for vine crops; Morgan (S7): "Sweet pepper is also
  considered a high light crop, however, slightly lower DLI are required for maximum
  production than tomato."
- saturation: not found.
- photoperiod: day-neutral (`GRP`, Runkle, as for tomato).

**cucumber: 15 / 25 / 30**

- min 15 (`GRP`): Runkle (S4), vine crops, as for tomato.
- target 25 (`MID`) and saturation 30 (`RH`): Virginia Tech (S5): "Cucumber 20−30".
  Dorais (S6): "Under a high light integral (30 mol m-2 d-1 or more) the growing
  period was only 10 days compared to 24 and 17 days under 5.5 and 10 mol m-2 d-1,
  respectively." The text does not say which growing period; it reads as the young
  plant stage.
- photoperiod: day-neutral (`GRP`, Runkle).

**zucchini: 20 / 25 / 30**

- Virginia Tech (S5): "Zucchini 20−30". `RL`, `MID`, `RH`. No second source found.

**strawberry: 12 / 22.5 / 30**

- Ohio State, Kubota lab (S8): "We consider minimum and optimum DLI inside the
  greenhouse for strawberry as 10-12 mol/m2/d and 20-25 mol/m2/d, respectively."
  and "DLI exceeding 30 mol/m2/d seems to impart stress to strawberry plants."
- min 12: the upper end of the stated minimum, chosen so the game does not treat
  10 as comfortable. target 22.5 (`MID` of 20 to 25). saturation 30 (harm
  threshold).
- photoperiod, cultivar-dependent (S8b): "Many cultivars used worldwide are
  short-day type (aka June-bearing type)." and "Ever-bearing strawberry cultivars
  (varieties) (including those classified in day-neutral cultivar group) are
  generally known as facultative long-day plants".

### Roots and tubers

**radish: 12 / 13.5 / 15**

- Currey and Konjoian, Iowa State (S16), 2 to 22 tested: "approximately 12 to 15
  mol·m-2·d-1 is an optimal DLI for commercial radish production", "there is minimal
  return on providing light to exceed 15 mol·m-2·d-1", "as the DLI increases above
  20 mol·m-2·d-1, radish diameter and weight start to diminish", and "The
  percentage of damaged roots" (usually cracks, the article says) "increased as the
  DLI increased above 15 mol·m-2·d-1".
- min 12 (low end of the optimum), target 13.5 (`MID`), saturation 15 (minimal
  return and more cracking above it).

**potato: blank / 42.2 / blank**

- target 42.2 (`CO2`): NASA Biomass Production Chamber (S17), Table 2, the daily PAR
  at the highest potato productivity. The chamber ran CO2 at "1000 or 1200 µmol
  mol-1", and potato at 655 to 917 µmol·m-2·s-1 on a 12-hour photoperiod, extended
  to 16 hours at 65 days in one study.
- min: not found. saturation: not found; Wheeler et al. (S20) say leaf
  "photosynthesis for the cvs. in our study could have been increased even further
  with higher PPF" than the 800 µmol·m-2·s-1 they used (34.6 mol·m-2·d-1 at 12 h).
- photoperiod (tuber-forming, not flowering) (S20): "most potatoes tuberize better
  under short photoperiods", and "late season cultivars like Kennebec may be more
  obligate for short days to promote tuberization, while early season cultivars like
  Norland may be more day neutral with regard to tuberization".

### Grains and grain legumes

**wheat: blank / 35.6 / 150**

- target 35.6 (`SB`): Ghosh et al., speed-breeding protocol (S21): "we recommend a
  photosynthetic photon flux density (PPFD) of approximately 450-500 µmol.m-2.s-1 at
  plant canopy height. Slightly lower or higher PPFD levels are also suitable." and
  "We recommend a photoperiod of 22 hours with 2 hours of darkness". 450 × 22 × 3600
  / 1,000,000 = 35.6; 500 gives 39.6. The low end is used because the protocol says
  lower is also suitable. For comparison, NASA's best wheat productivity (S17) was at
  67.0 (`CO2`, 20 to 24 h photoperiods).
- saturation 150 (`NR`, `CO2`): Bugbee and Salisbury, Utah State (S18), 22 to 150
  mol·m-2·d-1 at 1200 ppm CO2: crop growth rate and grain yield "both continued to
  increase up to the highest integrated daily PPF level, which was three times
  greater than a typical daily flux in the field."
- min: not found.
- photoperiod: long-day. Fu et al. (S22): "Compared to long-day crops (e.g., wheat,
  rapeseed), short-day crops require stricter photoperiod control."

**barley, oat, pea, chickpea: blank / 35.6 / blank**

- target 35.6 (`SB`): the same protocol (S21) names them among the crops it serves:
  "spring and winter bread wheat, durum wheat, barley, oat, various members of the
  Brassica family, chickpea, pea, grasspea, quinoa". Same arithmetic as wheat.
- photoperiod: long-day or day-neutral, inferred from the protocol's scope, which is
  "reducing the generation times for some long-day (LD) or day-neutral crops". It
  does not classify each crop.

**rice: blank / 43 / blank**

- target 43 (`SEC`): Conviron, a growth-chamber manufacturer (S23): "the optimal
  light conditions for rice grown in plant factories with artificial lighting
  (PFALs) are PPFDs of 1000 umol/m2/s and a 12 h photoperiod which realizes a Daily
  Light Integral (DLI) of 43 mol/m2/day", at "400 ppm" CO2, with yields "40-60%
  greater than the average paddy field yields in Japan". It cites a 2016 book chapter
  by E. Goto that was not read. (1000 × 12 × 3600 / 1,000,000 = 43.2.)
- photoperiod: short-day. Fu et al. (S22) describe rice's "short-day induced floral
  transition" and apply the method to "other short-day crops: soybean".

**soybean: blank / 36.5 / blank**

- target 36.5 (`CO2`): NASA (S17), Table 2, daily PAR at the highest soybean
  productivity (12 or 10 h photoperiods, CO2 1000 or 1200 ppm).
- photoperiod: short-day (S22, quoted under rice).

### Not applicable

**oyster_mushroom.** A fungus; it does not photosynthesise, so a daily light
integral has no meaning for its growth. (This is basic biology, not a sourced
figure. Any light a mushroom needs to form fruiting bodies is a signal, not an
energy supply; that was not researched here.)

### Not found (35 crops)

No crop-specific DLI, and no figure for a named group that includes the crop, in
anything read: aloe_vera, bean, beet, broccoli, cabbage, calendula, carrot,
cauliflower, celery, chamomile, chive, comfrey, corn, eggplant, feverfew, garlic,
ginger, hop, leek, lemongrass, lentil, okra, onion, parsnip, peanut, rosemary, rye,
saffron, sorghum, st_johns_wort, sunflower, sweet_potato, turmeric, turnip, valerian.

Near misses, recorded so nobody re-reads them expecting a number:

- sweet_potato: Mortley et al. (S24) found "Storage root growth for Georgia Jet and
  T1-155 increased with light intensity" between 480 and 960 µmol·m-2·s-1, but the
  abstract gives no photoperiod, so no DLI can be computed.
- lentil and peanut: the speed-breeding protocol (S21) mentions both only as
  examples of earlier photoperiod work, not among the crops its light level serves.
- rye: a long-day cereal like wheat, barley and oat, but not named in the
  speed-breeding protocol, and no source gives "cereals" as a group a figure.
- cabbage, broccoli, cauliflower: kale and collards (same species) have figures
  above, but no source gives a figure for Brassica crops as a group.
- bean, corn, sorghum, sunflower, carrot, onion: searches for controlled-environment
  DLI studies found only unrelated work (intercropping, microgreens).

---

## Sources

Read on 2026-09-26 unless stated. "Via archive" means the live page blocked
automated reading and the Internet Archive copy named was read instead.

- **S1** Torres, A.P. and R.G. Lopez. *Measuring Daily Light Integral in a
  Greenhouse*, Purdue Extension HO-238-W. No date printed; PDF metadata January
  2010. Table 2 credited "Source: James E. Faust, Ball Red Book."
  https://www.extension.purdue.edu/extmedia/HO/HO-238-W.pdf
- **S2** Cornell University CEA Program. *Hydroponic Lettuce Handbook*, 2013.
  https://cpb-us-e1.wpmucdn.com/blogs.cornell.edu/dist/8/8824/files/2019/06/Cornell-CEA-Lettuce-Handbook-.pdf
- **S3** Brechner, M. and D. de Villiers. *Hydroponic Spinach Production Handbook*,
  Cornell University CEA Program, 2013.
  https://cpb-us-e1.wpmucdn.com/blogs.cornell.edu/dist/8/8824/files/2019/06/Cornell-CEA-baby-spinach-handbook.pdf
- **S4** Runkle, E. (Michigan State University). "Technically speaking: Lighting
  greenhouse vegetables", *Greenhouse Product News*, December 2011, p. 42.
  https://www.canr.msu.edu/uploads/resources/pdfs/lightingvegetables.pdf (via
  archive: https://web.archive.org/web/2022/https://www.canr.msu.edu/uploads/resources/pdfs/lightingvegetables.pdf)
- **S5** Stallknecht, E. *Calculating and Using Daily Light Integral (DLI): An
  Introductory Guide*, Virginia Cooperative Extension SPES-720, published August 13,
  2025. Table 3, "Recommended DLI range". Its values cite Dou et al. 2018, Faust et
  al. 2005 and Pramuk and Runkle 2005.
  https://www.pubs.ext.vt.edu/SPES/spes-720/spes-720.html
- **S6** Dorais, M. (Agriculture and Agri-Food Canada). "The use of supplemental
  lighting for vegetable crop production: light intensity, crop response, nutrition,
  crop management, cultural practices", Canadian Greenhouse Conference, October 9,
  2003. http://www.agrireseau.qc.ca/legumesdeserre/documents/cgc-dorais2003fin2.pdf
  (via archive, 2015 capture:
  https://web.archive.org/web/2015/http://www.agrireseau.qc.ca/legumesdeserre/documents/cgc-dorais2003fin2.pdf)
- **S7** Morgan, L. "Daily Light Integral (DLI) and greenhouse tomato production",
  *The Tomato Magazine*, Winter 2013, pp. 10-11 and 15. A trade magazine, not
  extension; used only for context alongside S4 to S6.
  https://www.specmeters.com/assets/1/7/2013_-_DLI_Greenhouse_Tomato1.pdf (via
  archive, 2020 capture)
- **S8** Kubota lab, The Ohio State University. *Controlled Environment Berry
  Production Information*, "Photosynthetic lighting" (no date shown).
  https://u.osu.edu/indoorberry/photosynthetic-lighting/ . **S8b** same site,
  "Flowering basics", "Updated by Chieri Kubota (Nov. 2025)".
  https://u.osu.edu/indoorberry/flowering-basics/
- **S9** Currey, C.J. and A.G. Litvin (Iowa State University). "Improve culinary
  herb yields", *Produce Grower*, July 17, 2019 (August 2019 issue).
  https://www.producegrower.com/article/hydroponic-production-primer-improve-culinary-herb-yields/
  (via archive, capture of 2019-08-18)
- **S10** Walters, K.J., S. Tarr and R.G. Lopez. "Modeling purple basil, sage,
  spearmint, and sweet basil responses to daily light integral and mean daily
  temperature", *PLOS ONE* 18(11): e0294905, published November 30, 2023. Its DLI
  optima for mint and sage cite Litvin, A.G., Iowa State University doctoral
  dissertation, 2019 (not read, see below).
  https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0294905
- **S11** Walters, K.J. and R.G. Lopez. "Modeling growth and development of
  hydroponically grown dill, parsley, and watercress in response to photosynthetic
  daily light integral and mean daily temperature", *PLOS ONE* 16(3): e0248662,
  published March 25, 2021.
  https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0248662
- **S12** Dou, H., G. Niu, M. Gu and J.G. Masabni. "Responses of sweet basil to
  different daily light integrals in photosynthesis, morphology, yield, and
  nutritional quality", *HortScience* 53: 496-503, April 2018.
  https://doi.org/10.21273/HORTSCI12785-17 (abstract read through the Crossref API;
  the journal site blocked automated reading)
- **S13** Currey, C.J. and J. Yost (Iowa State University). "Managing the Daily
  Light Integral for Leafy Greens", *Greenhouse Product News*, October 2020.
  https://gpnmag.com/article/managing-the-daily-light-integral-for-leafy-greens/
- **S14** Baumbauer, D.A., C.B. Schmidt and M.H. Burgess (Montana State
  University). "Leaf Lettuce Yield Is More Sensitive to Low Daily Light Integral
  than Kale and Spinach", *HortScience* 54: 2159-2162, December 2019.
  https://doi.org/10.21273/HORTSCI14288-19 (abstract via Crossref)
- **S15** Gagne, C.G. *The effects of daily light integral on the growth and
  development of hydroponically grown baby leaf vegetables*, Master of Professional
  Studies report, Cornell University, August 2019.
  https://ecommons.cornell.edu/server/api/core/bitstreams/fba7b9e9-2060-4665-b677-831c567a6fc8/content
- **S16** Currey, C.J. and P. Konjoian. "How Increasing Light Improves Radish
  Yields", *Greenhouse Grower*, October 10, 2023 (Iowa State study).
  https://www.greenhousegrower.com/production/how-increasing-light-improves-radish-yields/
- **S17** Wheeler, R.M., C.L. Mackowiak, G.W. Stutte, N.C. Yorio, L.M. Ruffe, J.C.
  Sager, R.P. Prince and W.M. Knott (NASA Kennedy Space Center). "Crop productivities
  and radiation use efficiencies for bioregenerative life support", *Advances in
  Space Research* 41: 706-713, 2008. Table 2 daily PAR (mol·m-2·d-1) at the highest
  productivity: wheat 67.0, soybean 36.5, lettuce 16.8, potato 42.2, tomato 38.6.
  http://bigidea.nianet.org/wp-content/uploads/2018/07/Adv-Space-Res-2008-Crop-Prod-and-Rad-Use-Eff.pdf
  (via archive, 2020 capture)
- **S18** Bugbee, B.G. and F.B. Salisbury (Utah State University). "Exploring the
  limits of crop productivity. I. Photosynthetic efficiency of wheat in high
  irradiance environments", *Plant Physiology* 88: 869-878, November 1988.
  https://academic.oup.com/plphys/article/88/3/869/6083363
- **S19** Frantz, J.M., G. Ritchie, N.N. Cometti, J. Robinson and B. Bugbee (Utah
  State University). "Exploring the limits of crop productivity: beyond the limits
  of tipburn in lettuce", *Journal of the American Society for Horticultural
  Science* 129: 331-338, May 2004. https://doi.org/10.21273/JASHS.129.3.0331
  (abstract via Crossref)
- **S20** Wheeler, R.M., A.H. Fitzpatrick and T.W. Tibbitts. "Potatoes as a Crop for
  Space Life Support: Effect of CO2, Irradiance, and Photoperiod on Leaf
  Photosynthesis and Stomatal Conductance", *Frontiers in Plant Science* 10: 1632,
  published December 19, 2019.
  https://www.frontiersin.org/journals/plant-science/articles/10.3389/fpls.2019.01632/full
- **S21** Ghosh, S., A. Watson, ... B.B.H. Wulff and L.T. Hickey (29 authors). "Speed
  breeding in growth chambers and glasshouses for crop breeding and model plant
  research", bioRxiv preprint 369512 v1, July 2018; published in *Nature Protocols*
  13: 2944-2963, December 2018. The preprint was read; the published version is
  paywalled and its wording may differ.
  https://www.biorxiv.org/content/10.1101/369512v1
- **S22** Fu, G. et al. "A Protocol to Shorten Rice Growth Cycle in Plant Factories:
  An Integrated Study of Light, Planting Density and Phytohormone Regulation",
  *Plants* 15(3): 343, published January 23, 2026. Used only for photoperiod class.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC12899853/
- **S23** Conviron. "Plant growth chambers for rice research", April 9, 2025. A
  manufacturer's article; its figure cites Goto, E. (2016), chapter 15 in Kozai, T.
  (ed.), *Plant Factory: An Indoor Vertical Farming System for Efficient Quality Food
  Production*, Elsevier. https://www.conviron.com/insights/plant-growth-chambers-for-rice-research/
- **S24** Mortley, D.G., C.K. Bonsi, W.A. Hill, P.A. Loretan and C.E. Morris.
  "Irradiance and nitrogen to potassium ratio influences sweetpotato yield in
  nutrient film technique", *Crop Science* 33: 782-784, July 1993.
  https://doi.org/10.2135/cropsci1993.0011183X003300040030x (abstract via Crossref)

---

## What is still unknown, and what would settle it

- **The real ceiling for the `NR` crops** (basil, cilantro, dill, oregano, thyme,
  kale, wheat). Every study found stopped at 20 to 22, and Currey and Yost (S13) say
  why: "providing supplemental light in a greenhouse or increasing lighting in a
  vertical farm to that degree is not practical or, likely, economical." Settle with
  Litvin's 2019 dissertation (free, but blocked to automated reading; a person with a
  browser can open it) and any later Iowa State or Purdue herb work above 22.
- **Ambient-air values for crops only measured in CO2-enriched chambers** (potato,
  soybean, NASA wheat). The home's air is not enriched, so these targets are
  probably higher than a home crop can use. Settle with ambient-CO2 controlled
  environment studies; Bugbee's lab at Utah State is the most likely publisher.
- **The 35 crops with nothing.** Brassicas other than kale, alliums, root crops other
  than radish, eggplant, okra, beans, maize, sorghum, sunflower and every medicinal
  herb. Settle with a targeted search of *HortScience*, *HortTechnology* and *Acta
  Horticulturae* (partly paywalled), the paid book López and Runkle (eds.), *Light
  Management in Controlled Environments* (2017), the Ball Red Book chart that
  Purdue's Table 2 reproduces, or by asking a controlled-environment extension
  specialist. Cost: hours of reading, or a book purchase.
- **Whether the home beds have downward airflow.** This decides whether lettuce's
  ceiling is 17 or 12. It is a game design question, not a research one.
- **Photoperiod class** is sourced for only 13 crops here (4 of them inferred); the others need a
  separate search if a later rung uses it.

## Sources that could not be reached

- Litvin, A.G. (2019), Iowa State dissertation, https://dr.lib.iastate.edu/etd/17042/
  : HTTP 403. Wanted: per-species saturating DLI for eight herbs.
- Yamori, W. et al. (2014), "Feasibility study of rice growth in plant factories",
  *Rice Research: Open Access* 2: 119,
  https://www.omicsonline.org/open-access/feasibility-study-of-rice-growth-in-plant-factories-2375-4338.1000119.php
  : timed out. Wanted: the primary source for the rice 43 figure.
- Goto, E. (2016), Elsevier book chapter: paid, not attempted.
- *Nature Protocols* published version of S21: paywall redirect; preprint read.
- journals.ashs.org article pages (S12, S14, S19, and Faust and Logan 2018,
  *HortScience* 53: 1250-1257): blocked to automated reading; abstracts only, via
  Crossref. The full texts may hold the per-crop tables the abstracts summarise.
- canr.msu.edu, specmeters.com, producegrower.com, agrireseau.net live pages:
  blocked or unreachable; Internet Archive copies read (named in the source list).
- ResearchGate and growingproduce.com: HTTP 403 (the radish study was read on
  Greenhouse Grower instead).

---

## What it means for this project

**Certain** (what the sources say):

- One DLI for every crop is wrong in both directions. Radish, parsley, mint and sage
  lose yield above about 15 to 16; fruiting crops are recommended 20 to 30; grains,
  soybean and potato under lamps were grown at 35 to 67.
- Lettuce at 17 needs air moved down onto the plants; in still air, 12.
- Tomato is stressed by continuous light and needs 4 to 6 hours of darkness a night
  (S4); some potato cultivars are intolerant of continuous light (S20).
- Oyster mushroom has no DLI.

**Judgement calls** (mine, open to disagreement):

- Using range ends and midpoints where no threshold was measured. The flags make
  every such choice visible, so a later reader can swap in a different rule.
- Treating `NR` saturations as floors. In a growth model, capping at an `NR` value
  under-rewards extra light slightly, which is the safer error for a teaching game.
- Using the speed-breeding level (35.6) as the lamp target for wheat, barley, oat,
  pea and chickpea. It is a real, sourced level these crops are grown to seed under,
  but it was chosen for speed, not yield.
- The home's grow lights add a night of light on top of a day of sun, so a lit plot
  is under continuous light. The sources above suggest tomato (and some potatoes)
  should do worse, not better, under that regime. Whether the game models this is a
  design decision.
- A blank value should not be filled with the old 17 silently; if the game needs a
  fallback, the crop card should say the number is unverified.

---

## The table

One row per crop; this is the table meant to be copied into game data. Blank means
not found. `photoperiod` is flowering response unless noted. Flags are defined in
"How the three numbers were chosen".

| id | dli_min | dli_target | dli_saturation | photoperiod | sources | flags |
|---|---|---|---|---|---|---|
| aloe_vera | | | | | | not found |
| barley | | 35.6 | | long-day or day-neutral (inferred) | S21 | SB |
| basil | 12.9 | 20 | 25 | | S12, S9, S10, S5 | sat RH; sources disagree |
| bean | | | | | | not found |
| beet | | | | | | not found |
| broccoli | | | | | | not found |
| cabbage | | | | | | not found |
| calendula | | | | | | not found |
| carrot | | | | | | not found |
| cauliflower | | | | | | not found |
| celery | | | | | | not found |
| chamomile | | | | | | not found |
| chickpea | | 35.6 | | long-day or day-neutral (inferred) | S21 | SB |
| chive | | | | | | not found |
| cilantro | 15 | 20 | 20 | | S5, S9 | min RL; sat NR |
| comfrey | | | | | | not found |
| corn | | | | | | not found |
| cucumber | 15 | 25 | 30 | day-neutral | S4, S5, S6 | min GRP; target MID; sat RH |
| dill | | 20 | 20 | | S9, S11 | sat NR |
| echinacea | 8 | 18 | | | S1 | ORN |
| eggplant | | | | | | not found |
| feverfew | | | | | | not found |
| garlic | | | | | | not found |
| ginger | | | | | | not found |
| hop | | | | | | not found |
| kale | 8 | 22 | 22 | | S14, S13, S15 | min LT; target NR; sat NR |
| lavender | 8 | 18 | | | S1 | ORN |
| leek | | | | | | not found |
| lemongrass | | | | | | not found |
| lentil | | | | | | not found |
| lettuce | 12 | 17 | 17 | day-neutral or long-day | S4, S6, S5, S2, S17, S19 | sat is a tipburn limit with airflow; 12 in still air |
| mint | 10 | 14.9 | 14.9 | | S9, S10 | min GRP |
| oat | | 35.6 | | long-day or day-neutral (inferred) | S21 | SB |
| okra | | | | | | not found |
| onion | | | | | | not found |
| oregano | | 20 | 20 | | S9 | sat NR |
| oyster_mushroom | | | | | | not applicable (fungus) |
| parsley | 10 | 15 | 15 | | S5, S9, S11 | min RL |
| parsnip | | | | | | not found |
| pea | | 35.6 | | long-day or day-neutral (inferred) | S21 | SB |
| peanut | | | | | | not found |
| pepper | 12 | 22 | | day-neutral | S6, S1, S4, S7 | |
| potato | | 42.2 | | tubers form better under short days | S17, S20 | CO2 |
| radish | 12 | 13.5 | 15 | | S16 | min RL; target MID |
| rice | | 43 | | short-day | S23, S22 | SEC |
| rosemary | | | | | | not found |
| rye | | | | | | not found |
| saffron | | | | | | not found |
| sage | 10 | 15.9 | 15.9 | | S9, S10 | min GRP |
| sorghum | | | | | | not found |
| soybean | | 36.5 | | short-day | S17, S22 | CO2 |
| spinach | 14 | 17 | 20 | | S5, S3, S14 | min RL; sat RH (22 with CO2) |
| st_johns_wort | | | | | | not found |
| strawberry | 12 | 22.5 | 30 | cultivar-dependent: June-bearing short-day; everbearing facultative long-day | S8, S8b | target MID |
| sunflower | | | | | | not found |
| sweet_potato | | | | | | not found |
| thyme | 8 | 20 | 20 | | S1, S9 | min ORN; sat NR |
| tomato | 15 | 25 | 30 | day-neutral | S4, S5, S6, S1, S7 | min GRP; target MID; sat RH |
| turmeric | | | | | | not found |
| turnip | | | | | | not found |
| valerian | | | | | | not found |
| wheat | | 35.6 | 150 | long-day | S21, S18, S17, S22 | target SB; sat NR, CO2 |
| zucchini | 20 | 25 | 30 | | S5 | RL, MID, RH |

**Again: this is a reading of public sources for a game, not agronomic advice.**
Where a real garden is concerned, the local extension service outranks this file.
