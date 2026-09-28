# Does a household Wi-Fi router harm plants in the same home?

**Research date: 2026-09-27.** Every source below was read on 2026-09-27. This is
a reading of public sources, done so a decision about the game's garden can rest on
what the evidence says. **It is not agronomic, engineering or health advice**, and
nobody who wrote it is a plant physiologist or a radio engineer. Where a source is
in Danish, Norwegian or Dutch, the quotation is the original and the English after
it is our own translation, marked as such.

**The question.** Does the radio emission of an ordinary household Wi-Fi router
harm plants growing in the same home, and if there is any effect, how large is it,
at what exposure, and in which plants?

Prepared for the operator's decision on the game's RF crop harm
(`src/systems/farming/mod.rs`, `RF_HARM_THRESHOLD` and `RF_HEALTH_PENALTY`), which
cites no source. What that code does on this date is described under "What this
means for the game" at the end.

**Decision, 2026-09-27.** The operator chose option 4, removal: "We'll assume no
wi-fi crop harm at this time." The same day the harm was taken out of the game:
the FarmingSystem's home-RF drain (`RF_HARM_THRESHOLD`, `RF_HEALTH_PENALTY`), the
`RfEmitter` component and its spawn, the `rf_emission` field on machine
definitions, and the buildability report's warning that a Wi-Fi data link can
harm a grow. The Wi-Fi router stays in the game as a powered network device, and
a Wi-Fi link still carries its bandwidth over its range like any other medium.
A test (`powered_wifi_router_leaves_crop_health_unchanged` in
`src/systems/farming/mod.rs`) holds that a powered router beside a crop changes
nothing about its health. The findings below are unchanged; they describe the
evidence as read on this date.

---

## Short answer

- **No source found shows a household router killing plants, or a garden, at the
  distances plants normally sit from a router in a home.** The game's mechanic
  (every crop in the house, of every species, well watered, dies within a fraction
  of an in-game day) is far outside anything any source reports.
- **There is a real, disputed literature of small laboratory studies** reporting
  that weak radio fields change plant biochemistry, gene activity and sometimes
  growth. The effects reported near an actual Wi-Fi router are: pea seedlings at
  43% and broccoli at 86% of control dry weight after a month at 8 to 30 cm; chilli
  germination at 75% at 5 cm; lettuce roughly 9% lighter at 1 m from two routers
  plus a cordless-phone base; parsley, dill and celery with altered leaf anatomy and
  scent chemistry at a field a router gives at about a third of a metre.
- **Other studies found nothing, or the opposite.** Onion germination was unchanged
  even at fields tens to thousands of times a router's at 1 m; young trees beside six
  routers for five to eight months showed no difference in growth, girth, leaf count
  or chlorophyll; beans under a 915 MHz field of about the power density a router
  gives at 1 m grew 19% taller and 20% heavier (with 18% fewer flower clusters); cress
  germinated normally beside a router in the one laboratory replication of the
  Danish school experiment.
- **Most of the underlying studies are rated poor by the reviewers who have graded
  them.** An Australian government systematic map scored 66% of plant studies as
  methodologically poor and 2% as good, and its follow-up found that poorer studies
  tend to report bigger effects. Halgamuge's often-quoted "89.9%" is the share of
  experimental observations reporting some physiological or morphological effect;
  its abstract neither separates harm from other change nor grades study quality.
- **Exposure matters and is usually overstated.** Most positive studies used fields
  that a typical 100 mW router only produces within about half a metre while it is
  transmitting. Routers transmit a few percent of the time, so a plant 1 to 2 m away
  receives on average roughly a hundred to several thousand times less than those
  studies used.
- **No international body sets an exposure limit for plants.** ICNIRP and WHO write
  for human health. ICNIRP has a statement on plants and animals in preparation
  (its 2024 to 2028 work plan); it was not published as of this date.
- **The 2013 Danish cress experiment** was a ninth-grade science-fair project, not a
  scientific study: no blinding, one location per treatment, three computers placed
  beside the exposed trays, and a stated aim to demonstrate harm. Its own numbers
  show about 24% fewer fully grown sprouts and a delayed start, not dead trays.
  Informal repeats reported in the press mostly found no effect, and the laboratory
  partial replication found no effect on cress germination.

---

## How to read this document

- **Units.** Power density S in milliwatts per square metre (mW/m2); electric field E
  in volts per metre (V/m). They convert by S = E x E / 377 (the impedance of free
  space in ohms). 1 microwatt per square centimetre (uW/cm2) = 10 mW/m2.
- **A router's field is calculated, not measured,** from its radiated power (EIRP)
  with the free-space far-field formula S = EIRP / (4 x pi x r x r). This is the
  standard first estimate. Real rooms differ by a few decibels either way from
  reflections, walls, antenna pattern and beamforming. One study below (Keller 2025)
  measured 8 mW/m2 at 1 m from routers, which agrees with the formula.
- **"While transmitting" vs "time-averaged".** A router sends in bursts. The
  measured share of time an access point actually transmits is given in finding
  E5. Many studies exposed plants continuously.
- **Frequency differs between studies.** Several studies used 900 MHz (mobile phone
  band); Wi-Fi is 2.4 GHz and 5 GHz. Power density is the same unit at every
  frequency, but how much a plant absorbs is not, so a 900 MHz result does not
  transfer to Wi-Fi one for one.
- **Quotations** reproduce the source's words exactly, including its spelling
  slips (marked [sic]). Where PDF text extraction garbled a glyph (degree signs,
  dashes inside numbers, quote marks, accented letters), it is written in plain text
  or restored; nothing else is changed. Values read off a graph by eye are labelled
  "read by eye".

---

## Findings

### A. Reviews and evidence maps

**A1. Halgamuge (2017), the review the brief names.** Halgamuge MN, "Review: Weak
radiofrequency radiation exposure from mobile phone radiation on plants",
*Electromagnetic Biology and Medicine* 36(2):213-235. Online 2016-09-20.
DOI [10.1080/15368378.2016.1220389](https://doi.org/10.1080/15368378.2016.1220389).
Abstract read (via Europe PMC); the full text is paywalled and was not read.

> "we performed an analysis of the data extracted from the 45 peer-reviewed
> scientific publications (1996-2016) describing 169 experimental observations"

> "Our analysis demonstrates that the data from a substantial amount of the studies
> on RF-EMFs from mobile phones show physiological and/or morphological effects
> (89.9%, p < 0.001). Additionally, our analysis of the results from these reported
> studies demonstrates that the maize, roselle, pea, fenugreek, duckweeds, tomato,
> onions and mungbean plants seem to be very sensitive to RF-EMFs."

> "Nonetheless, this endorses the need for more experiments to observe the effects
> of RF-EMFs, especially for the longer exposure durations, using the whole
> organisms."

What it shows: across 45 papers, most observations reported *some* change. What it
does not show, from the abstract: whether those changes were harmful, which way
they went, or how good the studies were. Its scope is "mobile phone radiation", and
some studies of that kind used a handset as the source (B5; see A2 on why that
set-up is hard to interpret); how many of the 169 observations came from such
set-ups cannot be told from the abstract. No published critique specific to this
review was found.

**A2. Vian, Davies, Gendraud and Bonnet (2016), a review from one of the groups
reporting effects.** "Plant Responses to High Frequency Electromagnetic Fields",
*BioMed Research International* 2016:1830262. Published 2016-02-14.
[PMC4769733](https://pmc.ncbi.nlm.nih.gov/articles/PMC4769733/).

> "we propose to consider nonionizing HF-EMF radiation as a noninjurious, genuine
> environmental factor that readily evokes changes in plant metabolism."

> "While the long-term impact of these metabolic changes remains largely unknown"

On studies that use a phone as the source:

> "While this apparatus has the advantage of being simple and economical, it poses
> many limitations that may compromise the quality of the exposure."

The group's own wording has shifted: its 2008 tomato paper (B1) described the
response as resembling a wound response, "perceived by plants as an injurious
stimulus", while this 2016 review calls the radiation "noninjurious". Neither
reports plants dying from it.

**A3. Cucurachi et al. (2013), systematic review of ecological effects.** Cucurachi
S, Tamis WL, Vijver MG, Peijnenburg WJ, Bolte JF, de Snoo GR, "A review of the
ecological effects of radiofrequency electromagnetic fields (RF-EMF)", *Environment
International* 51:116-140. Online 2012-12-20. DOI
[10.1016/j.envint.2012.10.009](https://doi.org/10.1016/j.envint.2012.10.009).
Abstract read.

> "In 65% of the studies, ecological effects of RF-EMF (50% of the animal studies
> and about 75% of the plant studies) were found both at high as well as at low
> dosages. No clear dose-effect relationship could be discerned."

> "However, a lack of standardisation and a limited number of observations limit the
> possibility of generalising results from an organism to an ecosystem level."

**A4. Karipidis et al. (2023), systematic map by the Australian radiation agency
(ARPANSA).** "What evidence exists on the impact of anthropogenic radiofrequency
electromagnetic fields on animals and plants in the environment: a systematic map",
*Environmental Evidence* 12:9. Published 2023-05-11.
[PMC11378816](https://pmc.ncbi.nlm.nih.gov/articles/PMC11378816/). Full text read.

> "334 articles (237 on fauna and 97 on flora) that were relevant were included in
> the systematic map."

> "The majority of the studies were methodologically poor (59% fauna and 66% flora)
> and only a very small number of studies employed good quality methods (6% fauna
> and 2% flora)"

> "Most experimental studies employed a control/sham condition, especially when
> investigating plants, but lacked in dosimetry and temperature control and not many
> experimental studies used positive controls or blinding"

> "there are currently no recognised international guidelines to specifically
> protect animals and plants."

The quality criteria were "appropriate dosimetry, use of controls, use of positive
controls, use of blinding, use of temperature monitoring" (quoted from the map's
methods).

**A5. Brzozek et al. (2024), analysis of the same map's data.** Brzozek C, Mate R,
Bhatt CR, Loughran S, Wood AW, Karipidis K, "Investigating the impact of
anthropogenic radiofrequency electromagnetic fields on animals and plants in the
environment: analysis from a systematic map", *International Journal of
Environmental Studies* 81(5):2343-2358. Published 2024-07-30. DOI
[10.1080/00207233.2024.2375861](https://doi.org/10.1080/00207233.2024.2375861).
Open access, full text read.

> "The results indicated that quality score is more indicative of the magnitude of
> the effect size than exposure-level parameters or exposure duration."

> "For flora, there were no significant correlations between ES and exposure level
> or duration across the studies; so, although there was no decrease in the impact
> of RF EMF with higher exposure there was also no increase; again, this may be
> related to the quality of the studies."

> "When stratified by quality score, trends emerged that indicated that poor quality
> studies were likely to report higher effect sizes compared to average or
> good-quality studies."

> "A similar correlation was found for flora studies, but this was not found to be
> statistically significant"

> "An investigation of the effect size in relation to different exposure
> characteristics, as well as the quality of the studies, raises doubts on whether
> animals and plants are truly affected at levels below human exposure limits."

A5 states: "This project was funded by the Australian Government's Electromagnetic
Energy Program", and both A4 and A5 are led by staff of ARPANSA, the Australian
radiation regulator. A reader who distrusts government radiation agencies on this
subject should weigh that; a reader who distrusts the positive studies should weigh
the quality scores.

**A6. Levitt, Lai and Manville (2022), a review arguing for concern.** "Effects of
non-ionizing electromagnetic fields on flora and fauna, Part 2 impacts: how species
interact with natural and man-made EMF", *Reviews on Environmental Health*
37(3):327-406. Online 2021-07-08. DOI
[10.1515/reveh-2021-0050](https://doi.org/10.1515/reveh-2021-0050). Abstract read.

> "Numerous studies across all frequencies and taxa indicate that current low-level
> anthropogenic EMF can have myriad adverse and synergistic effects"

> "Effects have been observed in mammals such as bats, cervids, cetaceans, and
> pinnipeds among others, and on birds, insects, amphibians, reptiles, microbes and
> many species of flora."

This is the strongest statement of concern found. It does not give a plant exposure
threshold or a household-router finding in the abstract.

### B. Controlled experiments that report effects

**B1. Roux et al. (2008), tomato, 900 MHz at 5 V/m for 10 minutes.** "High
frequency (900 MHz) low amplitude (5 V m-1) electromagnetic field: a genuine
environmental stimulus that affects transcription, translation, calcium and energy
charge in tomato", *Planta* 227(4):883-891. Online 2007-11-20. DOI
[10.1007/s00425-007-0664-2](https://doi.org/10.1007/s00425-007-0664-2). Abstract.

> "we exposed tomato plants (Lycopersicon esculentum Mill. VFN8) to low level
> (900 MHz, 5 V m(-1)) electromagnetic fields for a short period (10 min)"

> "Within minutes of electromagnetic stimulation, stress-related mRNA (calmodulin,
> calcium-dependent protein kinase and proteinase inhibitor) accumulated in a rapid,
> large and 3-phase manner typical of an environmental stress response."

> "their similarities to wound responses strongly suggests that this radiation is
> perceived by plants as an injurious stimulus."

Exposure was in a mode-stirred reverberation chamber, the best-characterised set-up
in this literature. The outcome is molecular, measured within an hour; growth and
yield were not measured.

**B2. Grémiaux et al. (2016), rose, 900 MHz at 5 and 200 V/m.** "Low-amplitude,
high-frequency electromagnetic field exposure causes delayed and reduced growth in
Rosa hybrida", *Journal of Plant Physiology* 190:44-53. Online 2015-11-17. DOI
[10.1016/j.jplph.2015.11.004](https://doi.org/10.1016/j.jplph.2015.11.004). Abstract.

> "We observed no growth modification whatsoever exposure was performed on the
> 5-leaf stage plants." [sic]

> "In contrast, Axis II produced at the top of Axis I, that came from post-formed
> secondary buds consistently displayed a delayed and significant reduced growth
> (45%)."

> "The measurements of plant energy uptake from HF-EMF in this exposure condition
> (SAR of 7.2 10(-4)Wkg(-1)) indicated that this biological response is likely not
> due to thermal effect."

A reported growth effect, from the best-controlled set-up, confined to one class
of shoots on young rooted cuttings; plants exposed at the 5-leaf stage showed none.

**B3. Halgamuge, Yak and Eberhardt (2015), soybean.** "Reduced growth of soybean
seedlings after exposure to weak microwave radiation from GSM 900 mobile phone and
base station", *Bioelectromagnetics* 36(2):87-95. Online 2015-01-21. DOI
[10.1002/bem.21890](https://doi.org/10.1002/bem.21890). Abstract.

> "The exposure to higher amplitude (41 V m(-1)) GSM radiation resulted in
> diminished outgrowth of the epicotyl. The exposure to lower amplitude
> (5.7 V m(-1)) GSM radiation did not influence outgrowth of epicotyl, hypocotyls,
> or roots."

> "Soybean seedlings were also exposed for 5 days to an extremely low level of
> radiation (GSM 900 MHz, 0.56 V m(-1)) and outgrowth was studied 2 days later.
> Growth of epicotyl and hypocotyl was found to be reduced, whereas the outgrowth of
> roots was stimulated."

Mixed in direction and not monotonic with dose: for the GSM signal, the strongest
field (41 V/m) and the weakest (0.56 V/m) both changed growth while 5.7 V/m did not.
The same abstract reports that the unmodulated (CW) signal reduced root growth at
the higher amplitude and hypocotyl growth at the lower one.

**B4. Tkalec, Malarić and Pevalek-Kozlina (2007), duckweed.** "Exposure to
radiofrequency radiation induces oxidative stress in duckweed Lemna minor L.",
*Science of the Total Environment* 388(1-3):78-89. Online 2007-09-07. DOI
[10.1016/j.scitotenv.2007.07.052](https://doi.org/10.1016/j.scitotenv.2007.07.052).
Abstract.

> "Duckweed was exposed for 2 h to EMFs of 400 and 900 MHz at field strengths of
> 10, 23, 41 and 120 V m(-1)."

> "Our results showed that non-thermal exposure to investigated radiofrequency
> fields induced oxidative stress in duckweed as well as unspecific stress
> responses, especially of antioxidative enzymes. However, the observed effects
> markedly depended on the field frequencies applied as well as on other exposure
> parameters (strength, modulation and exposure time)."

**B5. Sharma et al. (2010), mung bean, a phone as source.** Sharma VP, Singh HP,
Batish DR, Kohli RK, "Cell phone radiations affect early growth of Vigna radiata
(mung bean) through biochemical alterations", *Zeitschrift für Naturforschung C*
65(1-2):66-72. DOI [10.1515/znc-2010-1-212](https://doi.org/10.1515/znc-2010-1-212).
Abstract.

> "We investigated the impact of cell phone electromagentic field (EMF) radiations
> (power density, 8.55 microW cm(-2)) on germination, early growth" [sic]

> "Cell phone EMF radiations significantly reduced the seedling length and dry
> weight of V radiata after exposure for 0.5, 1, 2, and 4 h."

8.55 uW/cm2 is 85.5 mW/m2.

**B6. Soran et al. (2014), parsley, dill and celery, with an actual Wi-Fi router.**
Soran M-L, Stan M, Niinemets Ü, Copolovici L, "Influence of microwave frequency
electromagnetic radiation on terpene emission and content in aromatic plants",
*Journal of Plant Physiology* 171(15):1436-1443. Issue date 2014-09-15. DOI
[10.1016/j.jplph.2014.06.013](https://doi.org/10.1016/j.jplph.2014.06.013);
[PMC4410321](https://pmc.ncbi.nlm.nih.gov/articles/PMC4410321/). Full text read.

> "a D-LINK wireless router 802.11g/2.4 GHz (2.412 – 2.48 GHz frequency range,
> Pout 19 dBm)"

> "The exposure levels where chosen in agreement with the microwave irradiation
> levels measured in open space for heavily used GSM networks (100mW/m 2 ) and for
> indoor WLAN (70mW/m 2 ) communication protocols." [sic]

> "One chamber was for non-treated control plants, while plants in the other two
> chambers were subjected to microwave irradiation."

> "Microwave irradiation resulted in thinner cell walls, smaller chloroplasts and
> mitochondria, and enhanced emissions of volatile compounds, in particular,
> monoterpenes and green leaf volatiles. These effects were stronger for
> WLAN-frequency microwaves. Essential oil content was enhanced by GSM-frequency
> microwaves, but the effect of WLAN-frequency microwaves was inhibitory."

Three weeks of continuous exposure, eight plants measured per group, temperature
held at 25 C in all chambers. One chamber per treatment means a chamber difference
cannot be separated from the radio difference (our reading of the design, not the
authors' statement). The paper reports anatomy, scent chemistry and photosynthesis
parameters, not yield or death.

**B7. Havas and Symington (2016), four species beside a Wi-Fi router, the
laboratory partial replication of the Danish experiment.** "Effects of Wi-Fi
Radiation on Germination and Growth of Broccoli, Pea, Red Clover and Garden Cress
Seedlings: A Partial Replication Study", *Current Chemical Biology* 10(1):65-73,
2016. DOI [10.2174/2212796810666160419161000](https://doi.org/10.2174/2212796810666160419161000).
The publisher's page sits behind a bot check; the full text was read from a copy
filed as exhibit R-30 in a Quebec class action, archived at
[web.archive.org](http://web.archive.org/web/20240621051513/http://collectiveactionquebec.com/uploads/8/0/9/7/80976394/exhibit_r-30__2016_havas_symington_pea_wifi_ccb.pdf).

> "One set of seeds was placed in Petri plates in a germination chamber kept under
> controlled conditions and was exposed to microwave radiation generated by a Wi-Fi
> router (mean and maximum exposures 20–40 and 96 mW/m2 respectively)."

> "The radiation from the Wi-Fi router did not affect germination of any of the
> species tested. However, there was a significant reduction in dry weight of the
> broccoli (86% of control) and peas (43% of control) exposed to Wi-Fi radiation at
> the end of the experiment (p<0.01)."

> "Several small plants began to die and mould developed in those Petri plates."

From the methods and results:

> "The distance from the router ranged from 8 to 30 cm"

> "Note, this router was not used for Wi-Fi communication and the only signal was the
> beacon signal transmitted by routers when they are plugged into an electric
> outlet."

> "the other set was placed in an RF-shielded room that had one Wi-Fi router"

> "Temperature ranged from 20-23 C and 21-24 C in the reference and Wi-Fi exposed
> chambers respectively."

> "dry-weight biomass was statistically lower for broccoli (p<0.01) with Wi-Fi
> exposure but not for clover or cress."

> "A one-tailed student T-test was used to determine statistical significance
> assuming unequal variance."

> "While the greatest difference between the two treatments (with and without Wi-Fi)
> was the microwave radiation, ELF magnetic and electric fields were also higher in
> the presence of the router and hence could contribute to some of the effects
> observed."

> "The study was funded by MH."

The strongest router-specific growth result found. Its limits, all recorded in the
paper itself: one room or chamber per treatment, the exposed chamber about 1 C
warmer, other fields also higher, a one-tailed test (which assumes the direction in
advance), and plants within 30 cm of the router. We found no replication of it by
another group.

**B8. Nikalje and Rajam (2021), chilli, seeds 5 cm from a router.** "Wi-Fi
Radiation Negatively Influences Plant Growth and Biochemical Responses of Capsicum
annuum L var. Pusa Jwala", *Current Chemical Biology* 15(2):182-187. DOI
[10.2174/2212796814999201228193703](https://doi.org/10.2174/2212796814999201228193703).
Abstract read via the Semantic Scholar API (the publisher page is behind a bot check).

> "For the germination experiment, Chilli seeds were kept in close vicinity (5 cm)
> of a Wi-Fi router for 10 days."

> "Control seeds/plants were kept in another room with almost identical conditions
> like light, temperature, etc."

> "The seed germination in the vicinity of the Wi-Fi router was reduced to 75% and
> other growth-related parameters like root and shoot length, leaf length, leaf
> width, leaf area index and fresh weight were significantly reduced."

No field strength is given in the abstract. At 5 cm a plant is in the router's near
field and next to its warm casing; the control was in a different room.

**B9. Keller, Geier and Tran (2025), lettuce 1 m from two routers and a cordless
phone base.** "In-Depth Analysis of Chlorophyll Fluorescence Rise Kinetics Reveals
Interference Effects of a Radiofrequency Electromagnetic Field (RF-EMF) on Plant
Hormetic Responses to Drought Stress", *International Journal of Molecular Sciences*
26(15):7038. Published 2025-07-22. DOI
[10.3390/ijms26157038](https://doi.org/10.3390/ijms26157038);
[PMC12345933](https://pmc.ncbi.nlm.nih.gov/articles/PMC12345933/). Full text read.

> "The RF-EMF was generated by two Wi-Fi routers (Fritzbox 7530) with an integrated
> DECT base station and two DECT phones"

> "Radiation in the 1880–1900 MHz and 2.4 GHz frequency ranges, measured with the
> HF59B high-frequency analyzer (covering 700 MHz to 2.7 GHz), was 8000 μW/m 2
> (peak measurement)."

> "The RF-EMF-exposed plants were positioned in a circular arrangement at a distance
> of one meter from the emitters"

> "Lettuce plants were grown in an RF-EMF-free environment until they were three
> weeks old. The OJIP curves were measured at the start of treatment (null
> measurement) and twice within a period of ten days."

The exposure was outdoors, in the association's test field, not indoors.

> "The control plants ( Figure 11 ) placed adjacent to the RF-EMF-exposed area were
> shielded from the adjoining RF-EMF emitters with a fine-mesh metal fence (mesh size
> 13 mm and height 120 cm)"

> "In both experiments, the FM and DM of the control group were significantly higher
> than those of all the other groups (D, E, and ED), suggesting that RF-EMF exposure
> and drought treatment both negatively impacted plant growth."

> "Our results suggest that exposure to RF-EMFs weakens the plant's hormetic
> responses induced by drought treatment, both in terms of the response's magnitude
> and its extent."

Read by eye from the paper's Figure 9 (normalised, pooled, n=9): the RF-exposed
group's fresh and dry matter are both about 0.9 of control. So at a realistic
household distance, over about ten days, this study reports roughly a 9% smaller
lettuce, not a dead one.
The control sat behind a metal fence the exposed plants did not have, which may also
change light and air around them (our observation). The authors are at Forschungsring
e.V. and TU Darmstadt; funding was the Software AG Stiftung.

**B10. Djamai et al. (2026), parsley and coriander under continuous Wi-Fi.** "Impact
of Continuous WIFI Electromagnetic Radiation Exposure on Nutritional Quality and
Metabolic Responses in Parsley (Petroselinum crispum L.) and Coriander (Coriandrum
sativum L.) Seedlings", *Russian Journal of Plant Physiology*, published 2026-04-14.
DOI [10.1134/S1021443725608109](https://doi.org/10.1134/S1021443725608109).
Abstract read (Springer page).

> "The exposure of WIFI microwave from the vegetative growth stage exhibited a
> moderate metabolic alterations in parsley and coriander."

> "Exposure at the early stages also stimulated the production of polyphenolic
> compounds, enhanced the antioxidant capacity and improved the antibacterial
> activity."

Changes in both directions; no death or yield loss reported in the abstract.

**B11. Cammaerts and Johansson (2015), cress near two mobile masts.** "Effect of
man-made electromagnetic fields on common Brassicaceae Lepidium sativum (cress
d'Alinois) seed germination: a preliminary replication study", *Phyton,
International Journal of Experimental Botany* 84(1):132-137. DOI
[10.32604/phyton.2015.84.132](https://doi.org/10.32604/phyton.2015.84.132).
Abstract read (publisher page).

> "Under high levels of radiation (70-100 µW/m2 =175 mV/m), seeds of Brassicaceae
> Lepidium sativum (cress d'Alinois) never germinated."

> "When removed from the electromagnetic field, seeds germinated normally."

Not Wi-Fi: the source was mobile masts about 200 m away (per Havas 2016's summary
of it). 70 to 100 uW/m2 is 0.07 to 0.1 mW/m2, about 1% of what a 100 mW router gives
at 1 m while transmitting. Havas and Symington (B7) had cress germinate normally at
20 to 40 mW/m2, several hundred times higher, so the two results cannot both be
general rules. Our inference, the same point the critic in D4 makes: if this result
held generally, cress would struggle to germinate on ordinary city windowsills. A
methodological critique is under D4.

### C. Controlled experiments reporting no effect, or effects in the other direction

**C1. Tkalec et al. (2009), onion germination unchanged at up to 120 V/m.**
"Effects of radiofrequency electromagnetic fields on seed germination and root
meristematic cells of Allium cepa L.", *Mutation Research* 672(2):76-81. Online
2008-11-05. DOI [10.1016/j.mrgentox.2008.09.022](https://doi.org/10.1016/j.mrgentox.2008.09.022).
Abstract.

> "Germination rate and root length did not change significantly after exposure to
> radiofrequency fields under any of the treatment conditions."

> "the percentage of mitotic abnormalities increased after all exposure treatments."

Same lab and fields as B4 (10 to 120 V/m, 400 and 900 MHz, 2 to 4 h). No change in
germination or root growth; a cell-division finding under the microscope.

**C2. Surducan et al. (2020), beans grew larger at a router-like power density.**
Surducan V, Surducan E, Neamtu C, Mot AC, Ciorîță A, "Effects of Long-Term Exposure
to Low-Power 915 MHz Unmodulated Radiation on Phaseolus vulgaris L.",
*Bioelectromagnetics* 41(3):200-212. Online 2020-02-06. DOI
[10.1002/bem.22253](https://doi.org/10.1002/bem.22253). Abstract.

> "The plants were grown in two separate electromagnetic field (EMF) shielded rooms"

> "with a maximum power density of 10 mW/m 2 measured near the plants"

> "The irradiated batch grew higher (19% increase in plant height, 20% increase in
> stem and leaves' dry mass), with 18% fewer inflorescences, and extremely long roots
> (34% increase in dry mass)."

Continuous exposure from sowing to maturity, at about the power density a 100 mW
router gives at 0.9 m while transmitting. The authors call the changes significant
morphological modifications; the direction for growth is up, for flowering down.
One room per treatment, as in B6 and B7.

**C3. Wageningen University (2013), young trees beside six Wi-Fi routers for five to
eight months.** van Lammeren AAM (project coordinator), Rapportage "Effect EM
Velden op bomen", Laboratorium voor Celbiologie, Wageningen UR, for the Productschap
Tuinbouw, project 14394. Dated November 2013. [edepot.wur.nl/309835](https://edepot.wur.nl/309835).
Full report read. In Dutch; translations ours.

> "Elektromagnetische velden zijn opgewekt door 6 WiFi-zenders (standaard routers)
> per klimaatcel met een vermogen van 100mW per zender in het frequentiegebied
> 2,4 GHz."

(Our translation: electromagnetic fields were generated by 6 Wi-Fi transmitters,
standard routers, per climate cell, at 100 mW per transmitter in the 2.4 GHz band.)

> "In één controle-opzet (alleen in 2011) is gekozen voor het aanbrengen van 6
> WiFi-zenders met dummy loads i.p.v. antennes om veronderstelde warmte-effecten en
> magnetische velden van de apparatuur uit te sluiten (sham-experiment)."

(Ours: in one control set-up, 2011 only, six Wi-Fi transmitters were fitted with
dummy loads instead of antennas, to exclude supposed heat effects and magnetic
fields from the equipment: a sham experiment.)

> "Samenvattend concluderen wij dat er onder de gegeven proefomstandigheden geen
> effect van EM-velden op getoetste bomen is vastgesteld op het niveau van
> lengtegroei, stamdiktegroei, het aantal bladeren, bladmorfologie en het
> bladchlorophyllgehalte (uitgezonderd een afwijkende waarde in nov 2012)."

(Ours: in summary, under the test conditions no effect of EM fields on the trees was
found in height growth, stem thickening, leaf number, leaf morphology or leaf
chlorophyll, apart from one deviating value in November 2012.)

> "De aspecten bladkrulling, metaalglans, exudaatvorming en epidermisnecrose vragen
> om verdere aandacht vanwege het beperkt aantal herhalingen."

(Ours: leaf curling, metallic sheen, exudate and epidermal necrosis need further
attention because of the limited number of repetitions.) The report found epidermal
necrosis significantly more often in cells with fields ("met een
overschrijdingskans van 0.4%", with a p-value of 0.4%), and says a repeat with the
sources moved between cells is needed to exclude other factors such as light
intensity.

> "Gegeven de beperktheid van de proefomvang en de beperktheid van het aantal
> herhalingen kunnen de resultaten slechts als indicatief worden geïnterpreteerd en
> kunnen geen stellige conclusies worden getrokken."

(Ours: given the limited size and number of repetitions, the results can only be
read as indicative and no firm conclusions can be drawn.)

Species: ash (170 trees), horse chestnut (83), willow (7). The measured Wi-Fi field in
the exposed cell was 0.1368 V/m (about 0.05 mW/m2), but the report states the
measurement was for comparing cells, not an absolute value: "Het doel van deze meting
is dan ook niet om de absolute waarde te bepalen maar de relatieve waarde" (ours:
the purpose was not to determine the absolute value but the relative one).

**C4. Informal repeats of the Danish experiment, gathered by the engineering weekly
Ingeniøren.** Møllerhøj J, "Læserne: Karsefrø spirer glimrende trods
mobilstråling", *Ingeniøren*, 2013-06-07.
[ing.dk](https://ing.dk/artikel/laeserne-karsefroe-spirer-glimrende-trods-mobilstraaling).

> "Det er ikke lykkedes for Ingeniørens læsere at forhindre karse i at gro ved at
> udsætte det for stråling fra mobiltelefoner og routere."

(Ours: Ingeniøren's readers did not manage to stop cress growing by exposing it to
radiation from mobile phones and routers.)

> "Forsøgene er ganske uvidenskabelige, og ikke direkte sammenlignelige med
> Hjallerup-forsøget, men hvorom alting er, så ser karsen ud til at klare sig."

(Ours: the tests are quite unscientific and not directly comparable with the
Hjallerup experiment, but whatever the case, the cress seems to cope.) One school's
test first showed slightly less growth on the router side, until a reader watching
the live stream noticed that side got more sun; after a sunshade went up, "Så
satte vi en solafskærmning op, og så så det ud til, at karsen kom sig" (ours: we put
up a sunshade, and then the cress seemed to recover). The same teacher was running a
second test with the router in the middle of the tray and heavier network load, and
at the time of writing said: "Jeg synes, man kan ane, at det spirer hurtigere ude i
enderne, end i midten" (ours: I think you can sense that it sprouts faster out at
the ends than in the middle), adding that it would be easier to say something
certain later. No follow-up report was found. All of this is anecdote, described as
such by the paper itself; it is included because it shows both a null result and
the kind of confounder (uneven sun) the Danish experiment was criticised for.

### D. The 2013 Danish school cress experiment

**D1. The students' own report (primary source).** Nielsen L, Coltau S, Nielsen S,
Nielsen M, Holm R (class 9.B, Hjallerup Skole), "Undersøgelse af non-termiske
effekter af mobilstråling", dated 2013-02-28, as published by DR, read from the
[Wayback Machine copy](http://web.archive.org/web/2013id_/http://www.dr.dk/NR/rdonlyres/075641A4-F4D4-4ECF-834F-C0DAF2B8E1E1/5134851/Undersoegelse_af_nontermiske_effekter_af_mobilstra.pdf)
(a scanned PDF; read page by page as images). Translations ours.

The stated aim:

> "Vi vil, med vores forsøg påvise, at karse som er udsat for EMR-stråling, har
> større risiko for at vokse mindre, eller dø, end karse som ikke er udsat for
> stråling."

(Ours: with our experiment we want to demonstrate that cress exposed to EMR has a
greater risk of growing less, or dying, than cress that is not.)

The set-up:

> "Vi startede med, at tælle 4800 karsefrø ud på 12 plastictallerkener, med 400 i
> hver."

> "De 12 tallerkener blev derefter sat i to forskellige vindueskarme med 6
> tallerkener i hver vindueskarm."

> "Derefter satte vi tre computere og to AP'ere op ved siden af karsen. De tre
> computere blev sat til konstant at kommunikere med hinanden, svarende til
> internetbrowsing."

(Ours: 4,800 cress seeds on 12 plastic plates, 400 each; the plates went on two
different windowsills, six per sill; then we set up three computers and two access
points next to the cress, and the three computers were set to communicate with each
other constantly, like internet browsing.)

The result and when it was measured:

> "Da vores kontrolgruppe søndag d. 24. februar havde nået sin maksimale højde,
> høstede vi mandag d. 25. februar vores tolv bakker karse."

> "Den bestrålede karse begyndte først at vokse flere dage efter kontrolgruppen
> gjorde."

(Ours: when our control group had reached its maximum height on Sunday 24 February,
we harvested all twelve trays on Monday 25 February. The irradiated cress only began
to grow several days after the control group did.)

The report's graph caption names the exposed trays as "udsat for mikrobølgestråling
fra computer og Wi-Fi" (exposed to microwave radiation from computer and Wi-Fi).
Read by eye from its graphs: fully grown sprouts per 400-seed tray about 300 to 372
in the control group and about 234 to 264 in the exposed group; biomass about 16 to
18 g against about 14 g. The report states no measured field strength in the pages
read.

**D2. Contemporary coverage.** DR (Danish public broadcaster), Bohn M, "Forsøg med
karse i 9. klasse vækker international opsigt", 2013-05-16
([dr.dk](https://www.dr.dk/nyheder/indland/forsoeg-med-karse-i-9-klasse-vaekker-international-opsigt)),
and DR Nordjylland / Ingeniøren, Kjeldsen N, "Tvivler på karseforsøg", 2013-05-17
([dr.dk](https://www.dr.dk/nyheder/regionale/nordjylland/tvivler-paa-karseforsoeg)).
The second quotes Per Kudsk, senior researcher in crop health at Aarhus University:

> "Men jeg har aldrig hørt om, at stråling i det niveau, som der må være tale om
> her, skulle påvirke planter så kraftigt."

(Ours: but I have never heard of radiation at the level that must be involved here
affecting plants so strongly.) ABC News (Bean D, 2013-05-24,
[abcnews.com](https://abcnews.com/blogs/technology/2013/05/can-wifi-signals-stunt-plant-growth))
quoted the supervising teacher, Kim Horsevad:

> "One would therefore generally be advised to await the results of his
> experiments before basing any important decisions on the outcome of the girls'
> experiment."

**D3. Critiques.** Norwegian science writer Gunnar Tjomlid, "Om karse,
wifi-stråling og en snurt naturfagslærer", blog, 2013-05-19
([tjomlid.com](https://tjomlid.com/2013/05/19/om-karse-wifi-straling-og-en-snurt-naturfagslaerer/)),
checked the report's numbers:

> "I den ubestrålte gruppen var det i snitt 332 fullvoksne spirer, og i den
> bestrålte gruppen var det i snitt 252."

(Ours: in the unexposed group there were on average 332 fully grown sprouts, and in
the exposed group on average 252.) Dutch writer Pepijn van Erp, "Danish School
Experiment with WiFi Routers and Garden Cress, Good Example of Bad Science", blog,
2013-05-25
([pepijnvanerp.nl](https://www.pepijnvanerp.nl/2013/05/danish-school-experiment-with-wifi-routers-and-garden-cress-good-example-of-bad-science/)),
summarising Tjomlid:

> "It's very likely that this had an effect on airflow and temperature around the
> plates and that could have an effect on germination, which has nothing to do with
> the presence of EM-fields."

> "The plates in a group were not separated in space, so we cannot regard the
> results of individual plates as independent observations. In fact, you could
> argue this is an N=2 experiment."

Both are blogs, not peer-reviewed, and both authors are openly sceptical of RF-harm
claims. The checkable facts they rely on (laptops beside the exposed trays, the 332
and 252 averages, harvest timed to the control group) match the students' report in
D1.

**D4. The two "replications".** Havas and Symington (B7) is the laboratory partial
replication; it found no effect of a router on cress germination and no difference
in cress biomass, and wrote: "We did not get the same results as the high schools
students in Denmark." [sic] Cammaerts and Johansson (B11) used mobile masts, not Wi-Fi. Van
Erp's critique of it, blog, 2016-01-04
([pepijnvanerp.nl](https://www.pepijnvanerp.nl/2016/01/cammaerts-and-johansson-manage-to-replicate-danish-garden-cress-wi-fi-experiment-with-even-more-mistakes/)):

> "They only used two (2!) trays with seeds 'nearby' the radiation source and
> another two somewhat further away."

**What kind of evidence it is.** A school science-fair project, careful for its
level, that DR reports won the students "en finaleplads i konkurrencen "Unge
Forskere"" (a finals place in the Young Researchers competition). It is not peer
reviewed, not blinded, has one location per treatment (so the six trays per group
are not independent), put heat-producing computers beside only the exposed trays on
a sunny windowsill, set the stopping day by the control group's growth, and set out
to demonstrate harm. Its own numbers show a delay and about 24% fewer fully grown
sprouts, not dead trays; the report's own side-by-side photographs are captioned
"efter 6 dage" (after 6 days), while the counts were made at day 13. In our
judgement it is an anecdote that prompted research, not a result.

### E. What a household router actually emits

**E1. UK Health Security Agency, "Wi-Fi radio waves and health", guidance, updated
2025-02-19.** [gov.uk](https://www.gov.uk/government/publications/wireless-networks-wi-fi-radio-waves-and-health/wi-fi-radio-waves-and-health).

> "The signals are very low power, typically 0.1 watt (100 milliwatts), in both the
> user device and the router (access point)."

**E2. The European limit: ETSI EN 300 328 V2.2.2 (2019-07),** the harmonised
standard for 2.4 GHz wideband data equipment.
[etsi.org PDF](https://www.etsi.org/deliver/etsi_en/300300_300399/300328/02.02.02_60/en_300328v020202p.pdf).

> "The RF output power is defined as the mean equivalent isotropic radiated power
> (e.i.r.p.) of the equipment during a transmission burst."

> "The RF output power for non-FHSS equipment shall be equal to or less than 20 dBm."

20 dBm is 100 mW. So in Europe a 2.4 GHz router radiates at most 100 mW while
transmitting.

**E3. The United States ceiling: 47 CFR 15.247(b)(3) and (b)(4),** read from
eCFR.gov (the section's last listed amendment is 85 FR 18149, 2020-04-01).

> "(3) For systems using digital modulation in the 902-928 MHz, 2400-2483.5 MHz, and
> 5725-5850 MHz bands: 1 Watt."

> "(4) The conducted output power limit specified in paragraph (b) of this section is
> based on the use of antennas with directional gains that do not exceed 6 dBi."

1 W into a 6 dBi antenna is 4 W EIRP. This is the legal maximum, not a typical
router.

**E4. Measured output of real access points: Peyman et al. (2011).** "Assessment of
exposure to electromagnetic fields from wireless computer networks (wi-fi) in
schools; results of laboratory measurements", *Health Physics* 100(6):594-612. DOI
[10.1097/HP.0b013e318200e203](https://doi.org/10.1097/HP.0b013e318200e203). Abstract.

> "These ranged from 3 to 28 mW for 12 access points at 2.4 GHz and from 3 to 29 mW
> for six access points at 5 GHz."

(Radiated power integrated over a hemisphere, because access points are wall
mounted.)

**E5. How much of the time an access point transmits: Khalid et al. (2011).**
"Exposure to radio frequency electromagnetic fields from wireless computer networks:
duty factors of Wi-Fi devices operating in schools", *Progress in Biophysics and
Molecular Biology* 107(3):412-420. Online 2011-08-16. DOI
[10.1016/j.pbiomolbio.2011.08.004](https://doi.org/10.1016/j.pbiomolbio.2011.08.004).
Abstract.

> "The duty factors of access points from 7 networks ranged from 1.0% to 11.7% with a
> mean of 4.79% (SD 3.76%)."

Measured in schools during lessons; a busy home streaming video could sit higher,
an idle one lower.

**E6. A multi-country survey: Foster (2007).** "Radiofrequency exposure from
wireless LANs utilizing Wi-Fi technology", *Health Physics* 92(3):280-289. DOI
[10.1097/01.hp.0000248117.74843.34](https://doi.org/10.1097/01.hp.0000248117.74843.34).
Abstract.

> "In all cases, the measured Wi-Fi signal levels were very far below international
> exposure limits (IEEE C95.1-2005 and ICNIRP) and in nearly all cases far below
> other RF signals in the same environments."

> "Important limiting factors are the low operating power of client cards and access
> points, and the low duty cycle of transmission that normally characterizes their
> operation."

### F. Standards bodies

**F1. ICNIRP (2020), "Guidelines for limiting exposure to electromagnetic fields
(100 kHz to 300 GHz)",** *Health Physics* 118(5):483-524. DOI
[10.1097/HP.0000000000001210](https://doi.org/10.1097/HP.0000000000001210);
[icnirp.org PDF](https://www.icnirp.org/cms/upload/publications/ICNIRPrfgdl2020.pdf).

> "THE GUIDELINES described here are for the protection of humans exposed to
> radiofrequency electromagnetic fields (EMFs) in the range 100 kHz to 300 GHz"

On non-thermal effects:

> "For the purpose of determining thresholds, evidence of adverse health effects
> arising from all radiofrequency EMF exposures is considered, including those
> referred to as 'low-level' and 'non-thermal', and including those where mechanisms
> have not been elucidated."

> "In summary, there is no evidence of effects of radiofrequency EMFs on
> physiological processes that impair human health."

Table 5 (reference levels, "averaged over 30 min and the whole body") gives, for the
general public, an incident power density of 10 W/m2 (10,000 mW/m2) above 2 GHz and
fM/200 W/m2 from 400 to 2000 MHz, which is 4.5 W/m2 (about 41 V/m) at 900 MHz. These
are human limits; ICNIRP does not claim they apply to plants.

**F2. ICNIRP work plan 2024-2028** (page undated, read 2026-09-27).
[icnirp.org](https://www.icnirp.org/en/activities/work-plan/index.html).

> "A Project Group was established to draft a Statement on environmental EMF
> protection on the basis of scientific papers of sufficient quality. And, if
> possible, to analyse whether the current human exposure guidelines are also
> sufficiently protective for plants and animals in their natural environment."

No such statement was found published as of this date. ICNIRP's earlier environment
work, the proceedings "Effects of Electromagnetic Fields on the Living Environment"
(Ismaning seminar, 1999; published 2000, with a chapter "Impacts of EMF on plants"
by G. Soja), is sold by ICNIRP and was not read.

**F3. WHO, "Electromagnetic fields and public health: Base stations and wireless
technologies", backgrounder, May 2006.**
[who.int](https://www.who.int/teams/environment-climate-change-and-health/radiation-and-health/non-ionizing/wireless).

> "Recent surveys have shown that the RF exposures from base stations range from
> 0.002% to 2% of the levels of international exposure guidelines, depending on a
> variety of factors such as the proximity to the antenna and the surrounding
> environment."

> "Considering the very low exposure levels and research results collected to date,
> there is no convincing scientific evidence that the weak RF signals from base
> stations and wireless networks cause adverse health effects."

This is about people. No WHO statement about plants was found.

---

## Exposure comparison, in the same units

### A household router, calculated

Free space, main beam, no walls. "While transmitting" is the burst level; the
time-averaged column uses the mean access-point duty factor of 4.79% (E5).

| Distance | 100 mW router, while transmitting | 100 mW router, time-averaged (4.79%) | 4 W (US legal ceiling), while transmitting |
|---|---|---|---|
| 0.3 m | 88 mW/m2 (5.8 V/m) | 4.2 mW/m2 | 3,540 mW/m2 (36 V/m) |
| 0.5 m | 32 mW/m2 (3.5 V/m) | 1.5 mW/m2 | 1,270 mW/m2 (22 V/m) |
| 1 m | 8.0 mW/m2 (1.7 V/m) | 0.38 mW/m2 | 318 mW/m2 (11 V/m) |
| 2 m | 2.0 mW/m2 (0.87 V/m) | 0.095 mW/m2 | 80 mW/m2 (5.5 V/m) |
| 3 m | 0.88 mW/m2 (0.58 V/m) | 0.042 mW/m2 | 35 mW/m2 (3.7 V/m) |
| 5 m | 0.32 mW/m2 (0.35 V/m) | 0.015 mW/m2 | 13 mW/m2 (2.2 V/m) |

For scale: the ICNIRP general-public reference level at 2.4 GHz is 10,000 mW/m2
(30-minute average). A 100 mW router at 1 m while transmitting is 0.08% of it;
time-averaged, about 0.004%. Walls between router and plants lower all of these
further. Keller 2025 measured 8 mW/m2 peak at 1 m (B9), matching the 1 m row.

### The studies, converted

The last-but-one column is the distance at which a 100 mW router, transmitting
continuously, would produce the study's level (same formula). "Near field" means
closer than the formula is valid for, where a real router would not produce a clean
field of that strength.

| Study | Plant | Source, frequency | Level as reported | mW/m2 (V/m) | Same level from a 100 mW router at | Exposure time | Outcome reported |
|---|---|---|---|---|---|---|---|
| Roux 2008 (B1) | tomato | reverberation chamber, 900 MHz | 5 V/m | 66 (5.0) | 0.35 m | 10 min | stress-gene transcripts within minutes |
| Grémiaux 2016 (B2) | rose | reverberation chamber, 900 MHz | 5 and 200 V/m | 66 and 106,000 | 0.35 m; near field | single exposure | 45% less growth of one shoot class on cuttings; none on 5-leaf plants |
| Halgamuge 2015 (B3) | soybean | TEM cell, 900 MHz | 5.7, 41 V/m; 0.56 V/m | 86, 4,460; 0.83 | 0.30 m, near field; 3.1 m | 2 h; 5 days | mixed: some parts shorter, roots longer |
| Tkalec 2007 (B4) | duckweed | TEM cell, 400 and 900 MHz | 10 to 120 V/m | 265 to 38,200 | 0.17 m and closer | 2 to 4 h | oxidative-stress markers |
| Tkalec 2009 (C1) | onion | same | 10 to 120 V/m | 265 to 38,200 | 0.17 m and closer | 2 to 4 h | germination and root length unchanged; mitotic abnormalities |
| Sharma 2010 (B5) | mung bean | phone | 8.55 uW/cm2 | 86 (5.7) | 0.31 m | 0.5 to 4 h | shorter seedlings, lower dry weight |
| Soran 2014 (B6) | parsley, dill, celery | Wi-Fi router, 2.4 GHz, chamber | 70 mW/m2 | 70 (5.1) | 0.34 m | 3 weeks | anatomy and scent chemistry changed; essential oil lower |
| Havas 2016 (B7) | pea, broccoli, clover, cress | Wi-Fi router beacon, 8 to 30 cm | 20 to 40 mean, 96 max | 20 to 96 (2.7 to 6.0) | 0.29 to 0.63 m | 28 to 30 days | no germination effect; pea 43% and broccoli 86% of control dry weight |
| Nikalje 2021 (B8) | chilli | Wi-Fi router at 5 cm | not stated | not stated | near field | 10 and 21 days | germination 75%; growth lower |
| Keller 2025 (B9) | lettuce | 2 routers + DECT at 1 m, outdoors | 8 mW/m2 peak (+2 at 5 GHz) | 8 (1.7) | 1.0 m | about 10 days | about 9% lighter (read by eye); weaker drought response |
| Surducan 2020 (C2) | bean | 915 MHz continuous | 10 mW/m2 max | 10 (1.9) | 0.89 m | sowing to maturity | 19% taller, 20% heavier, 18% fewer flower clusters |
| Wageningen 2013 (C3) | ash, chestnut, willow | 6 Wi-Fi routers (+UMTS, DVB-T) | 0.137 V/m (relative only) | about 0.05 | about 13 m | 5 to 8 months | no growth, girth, leaf or chlorophyll effect; some leaf symptoms to recheck |
| Cammaerts 2015 (B11) | cress | mobile masts, about 200 m | 70 to 100 uW/m2 | 0.07 to 0.1 (0.16 to 0.19) | 9 to 11 m | 10 days | no germination |
| Danish 2013 (D1) | cress | 2 access points + 3 computers beside trays | not stated | not stated | not known | 13 days | about 24% fewer full sprouts, later start |

What the table shows: the router-specific studies reporting clear growth loss (B7,
B8) placed plants within 30 cm of the router. The two studies at a realistic 1 m
power density (B9, C2) report changes of about 10 to 20%, in opposite directions.
The studies at 5 V/m and above correspond to standing a plant within about a third
of a metre of a router that never stops transmitting.

---

## What is still unknown, and what would settle it

1. **Whether a household router affects crop yield at ordinary distances (1 to 5 m,
   often through a wall) over a full crop cycle.** No source read tested this with
   blinding, several chambers per treatment, a sham router, and logged temperature.
   What would settle it: a pre-registered experiment with matched growth chambers
   (several per treatment, positions randomised), routers with antennas against
   routers on dummy loads (the Wageningen 2011 sham design, C3), measured field and
   temperature, and yield as the endpoint. Scale: a university horticulture project
   of the size Wageningen ran (C3 was three growing seasons).
2. **Whether reported changes are harm at all.** Directions disagree: smaller
   (B5, B7, B9), larger (C2), roots stimulated while shoots shrink (B3), antioxidants
   and antibacterial activity up (B10). Settled by the same kind of experiment
   measuring yield and quality, not biochemical markers.
3. **Which species, if any, are sensitive.** Halgamuge's list (maize, roselle, pea,
   fenugreek, duckweeds, tomato, onions, mungbean) comes from counting reported
   effects, and onion germination was unaffected in C1. No species has a published
   dose-response curve at Wi-Fi frequencies.
4. **5 GHz and 6 GHz Wi-Fi.** Limits and studies for these bands were not
   researched here, apart from Keller's 5 GHz reading (2 mW/m2 at 1 m).
5. **ICNIRP's environmental statement** (F2) was in preparation, not published. When
   it appears it will be the first standards-body position on plants; re-check the
   ICNIRP work-plan page.
6. **Mechanism.** None is established. The Vian group proposes calcium signalling
   (B1); nothing read explains how a field of a few mW/m2 would kill a plant.
7. **The full texts of Halgamuge 2017 (A1) and Levitt 2022 (A6)** were not read.
   Their per-study tables could show how many of the reported effects were growth
   losses at Wi-Fi-like exposures. Cost: library access or purchase.
8. **The Danish students' exposure level** is not in the report pages read (D1).

---

## Sources that could not be reached

- Snopes, "Do WiFi Signals Stunt Plant Growth?"
  ([snopes.com](https://www.snopes.com/fact-check/cress-wifi-experiment/)): HTTP 402.
  Hoped for: a summary of the contemporary debate. Not needed, since the students'
  report and the critiques were read directly.
- EurekaSelect / Bentham pages for Havas 2016 and Nikalje 2021: bot verification
  page; not bypassed. Havas read from the archived exhibit copy, Nikalje from the
  Semantic Scholar abstract.
- ResearchGate and Academia.edu copies of several papers: HTTP 403.
- PubMed web pages: a captcha page. Abstracts were read through Europe PMC instead.
- Halgamuge 2017 and Levitt 2022 full texts: paywalled; abstracts only.
- ICNIRP 2000 proceedings "Effects of Electromagnetic Fields on the Living
  Environment", including Soja G, "Impacts of EMF on plants": sold, not read.
- The EKLIPSE project's 2018 report on radiation and wildlife: known here only
  through Brzozek 2024's citation of it; not read.
- Ingeniøren's embedded copies of the students' poster and photo documentation
  failed to load ("Indholdet kan desværre ikke indlæses"). The report itself was
  read from DR's copy via the Wayback Machine.
- Wageningen's separate 2013 field-test report (van Kuik) and the bio-potential
  report (van 't Wout and Luik), known only through van Erp's 2014 blog summary.
- A 2024 student presentation, "Wi-Fry: Detrimental Impacts of Wi-Fi Router
  Radiation on Crop Seed Viability and Vigor" (a science-fair symposium listing),
  not read; not peer reviewed.

---

## What this means for the game

### Certain

- **What the code did when this was researched** (removed later the same day; see
  "Decision, 2026-09-27" at the top). `src/systems/farming/mod.rs` summed the
  `strength` of every powered `RfEmitter` in the world into one home-wide level. A
  `wifi_router` (`data/machines/home.ron`, `rf_emission: 0.6`; the shipped home
  places one as `router_1` in `room-study`) adds 0.6. Above `RF_HARM_THRESHOLD`
  (0.1), every crop loses `RF_HEALTH_PENALTY` (1.5) x 0.6 = 0.9 health per second
  on a 0 to 100 scale, against a well-watered recovery of 0.5 per second
  (`HEALTH_RECOVERY_RATE`). Net loss 0.4 per second: a healthy, watered crop reaches
  0 and dies in about 250 seconds of play. The drain uses the tick's unscaled `dt`,
  so how much of an in-game day that is depends on the clock's time scale; at a
  scale of 1 it is about a fifth of the 1,200-second day (our reading of the code).
  There is no distance, no wall and no species in the calculation. The code comment
  says "Sensitive crops lose health" but the drain applies to every crop. The code
  cites no source.
- **No source read reports a household router killing plants at household
  distances.** The only plant deaths near a router in the sources are seedlings in
  Petri dishes 8 to 30 cm from one for a month (B7) and some seedlings in the Danish
  school trays (D1; DR reported that some "var endda muterede eller døde", were even
  malformed or dead). By the students' own count, their exposed trays as a whole
  were delayed and about a quarter short of the control, not dead.
- **The largest router growth losses found were at under half a metre,** in single
  studies with one room or chamber per treatment, and were not replicated in
  anything read. At about 1 m, one study reports roughly 9% lighter lettuce and
  another, at a similar power density, beans 20% heavier.
- **There is no international exposure limit for plants,** and the body that would
  set one (ICNIRP) has not yet published on it. Human-health bodies (WHO, UKHSA)
  find no convincing evidence of harm to people from these levels; that says
  nothing directly about plants.

### Judgement call: the options, with what the evidence says for and against

The choice among these is the operator's. They are listed, not ranked.

**1. Keep the harm as it is.**
- For: the literature does contain reports of reduced plant growth near routers
  (B7, B8, B9), and the wired-against-wireless trade-off is a designed piece of play
  the operator asked for (`docs/design/telecom.md`).
- Against: the in-game effect (death of every crop, anywhere in the home, within a
  fraction of a day) is orders of magnitude beyond the largest effect any source
  reports; systematic reviewers rate most of the underlying studies poor (A4, A5);
  the game would teach as fact an effect that is disputed at close range and
  undemonstrated at household distance, which sits against the rule that the
  farming data teaches real growing.

**2. Scale it to what the evidence supports.**
- What "supported" could mean, using only the range the sources report: an effect
  confined to close range (tens of centimetres), falling with the square of
  distance and blocked by walls; small (on the order of 10% biomass at 1 m in one
  study, none or positive in others); never lethal; no reliable list of sensitive
  species.
- For: keeps the mechanic and the trade-off, and keeps the magnitude inside what
  studies report.
- Against: even a small, scaled effect presents a contested, unreplicated finding
  as settled, and the direction of change is not consistent between studies (C2,
  B10). Honesty would likely need an in-game note that the effect is disputed,
  and a distance model the farming system does not have today.

**3. Make it a setting.**
- For: fits the project's standing rule that deep systems ship a full-realism mode
  and a simplified mode with a plain toggle; a player who wants the wired-against-
  wireless trade-off can keep it, and the setting's text can state the evidence.
- Against: a toggle does not decide what the realism mode itself should do; if the
  setting defaults to on at today's strength, option 1's objections apply; if it
  defaults to off, the effect is effectively option 4 for most players.

**4. Remove it.** (Chosen by the operator on 2026-09-27; see the decision at the top.)
- For: the systematic map and its analysis raise doubt that plants are affected at
  levels below human limits (A4, A5); controlled studies found no growth effect on
  young trees beside six 100 mW routers for months (C3), no germination effect on
  onion at fields far above a router's (C1), and none on cress beside a router
  (B7); informal repeats of the Danish experiment mostly found nothing (C4).
- Against: poor-quality evidence of an effect is not good evidence of no effect;
  the best-controlled positive studies (B1, B2) do report plant responses at
  fields a router produces at about a third of a metre; ICNIRP has not ruled.
  Removing the crop harm need not remove the `RfEmitter` component, which also
  feeds the planned emissions-as-detection layer (`src/ecs/components.rs`,
  `docs/design/telecom.md`).

---

**Again: this is a reading of public sources for a game, not agronomic, engineering
or health advice.** Where a real garden or a real home is concerned, primary
research and qualified people outrank this file. Findings are dated so a later
reader can tell whether the science moved or this reading was wrong; if ICNIRP
publishes its environmental statement, or a replicated household-distance study
appears, write a new dated finding rather than editing this one.
