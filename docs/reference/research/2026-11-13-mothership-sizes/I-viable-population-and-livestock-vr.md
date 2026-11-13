# Helper notes I: The colony module's minimum viable population; VR for livestock (README sections 10 and 4)

Part of the [mothership sizes research](README.md) (2026-11-13). A research helper wrote these
notes on 2026-11-12 from the sources it could read; the README carries the answers and the
judgements. After the helper finished, every cited host's robots.txt was checked: findings
read from a page that refuses Claude's readers are marked REMOVED and their numbers are not used.
Quotations were spot-checked against their sources where the README says so; the rest are the
helper's readings, with the helper's own confidence. The helper's own robots notes and summary
table were written before that check: where they name a REMOVED finding or say a host allowed
reading, the check below the README's "How this was read" section is the one that holds.


Read 2026-11-12. Note: the WebSearch budget was already used up when this helper started, so every source was reached by direct DOI, arXiv, Crossref, Semantic Scholar or Europe PMC lookups, not by searching. Europe PMC and the fetch tool return paraphrased abstracts for most records, so most quotations are short fragments; each is marked.

## Robots checked
- arxiv.org: no ClaudeBot/Claude-User/anthropic-ai rule; wildcard allows /abs (crawl-delay 15). Read.
- pubmed.ncbi.nlm.nih.gov: wildcard group only, base article pages allowed (crawl-delay 1). Read (a search page only; no article found there).
- www.ebi.ac.uk (Europe PMC REST API): wildcard group, /europepmc not disallowed (crawl-delay 10). Read.
- www.cambridge.org: wildcard group, /core/journals not disallowed. Read.
- www.fws.gov: wildcard group blocks only /core/, /profiles/, /admin/, /search, /user/login. Read.
- www.sciencedirect.com: robots.txt and article page returned HTTP 403. NOT read.
- api.crossref.org, api.semanticscholar.org, api.openalex.org: metadata APIs (no robots.txt fetched for these; used only for bibliographic records, no pages scraped). Semantic Scholar's search endpoint answered 429 once; not retried.
- en.wikipedia.org: used only to find a primary source, not cited as one.
- www.bbc.com, www.newscientist.com: the fetch tool refused to fetch them ("unable to fetch"). NOT read.
- mcx.mosreg.ru (does not resolve) and msh.mosreg.ru (empty answer): the Moscow region ministry's own release could not be reached.

---
## TOPIC 1: minimum viable population

### I1. Marin and Beluffi 2018: minimal crew of 98
- The number(s): 98 people as the minimum crew for a 100 percent success rate over a roughly 6,300-year voyage (about 200+ generations); a start of 25 breeding pairs goes extinct in about half of runs if inbreeding is totally forbidden (paraphrase).
- Source: "Computing the minimal crew for a multi-generational space travel towards Proxima Centauri b", F. Marin and C. Beluffi, arXiv:1806.03856, https://arxiv.org/abs/1806.03856, source's own date: submitted 11 June 2018 (published later in JBIS)
- Quotation: "a minimum crew of 98 people is necessary" (the fetch tool shortened the quote of the abstract; the abstract's longer wording, "we find that a minimum crew of 98 people is necessary ensure a 100% success rate", was also returned by the tool as its own quote)
- Read: 2026-11-12
- Confidence: medium-high on the number (arXiv abstract page read twice); the model (HERITAGE Monte Carlo) uses managed breeding rules and yearly population review, so it is a floor for a well-run crew, not a safe general figure.
- Use for the ship: this is the low bound, valid only with strict, enforced pairing management and assumes no catastrophes. Use as a floor, never as the plan.

### I2. Frankham, Bradshaw and Brook 2014: Ne of 100 and 1,000
- The number(s): effective population size Ne at or above 100 to limit loss of total fitness to 10 percent or less (short term, inbreeding); Ne at or above 1,000 to keep long-term evolutionary potential. Replaces Franklin's 50 and 500.
- Source: "Genetics in conservation management: Revised recommendations for the 50/500 rules, Red List criteria and population viability analyses", R. Frankham, C.J.A. Bradshaw, B.W. Brook, Biological Conservation, doi:10.1016/j.biocon.2013.12.036 (via Europe PMC record; the publisher page returned 403/redirect only), source's own date: 2014
- Quotation: "with Ne⩾100 being required to limit loss in total fitness to ⩽10%." and "a better approximation is Ne⩾1000." (fragments returned by the fetch tool from the Europe PMC abstract record; also the tool's quote "Genetically effective population size (Ne)=50 is inadequate for preventing inbreeding depression")
- Read: 2026-11-12
- Confidence: high on the numbers (two consistent readings of the abstract record), medium on exact quotation wording (relayed by the fetch tool).
- Use for the ship: Ne 100 is the 5-generation floor, Ne 1,000 is the "indefinitely" target. Ne counts people who actually have children, weighted for unequal family sizes, not headcount.

### I3. Frankham 1995: Ne/N ratio in wildlife about 0.10 to 0.11
- The number(s): the ratio of effective to census (adult) population size averaged about 0.10 to 0.11 across 192 estimates from 102 species (wildlife has far smaller effective sizes than headcounts suggest). Factors that lower it: fluctuating size, uneven family size, unequal sex ratio.
- Source: "Effective population size/adult population size ratios in wildlife: a review", Richard Frankham, Genetical Research 66(2):95-107, https://www.cambridge.org/core/product/identifier/S0016672300034455/type/journal_article (doi 10.1017/S0016672300034455; online 2009; also reprinted in Genet Res 89, 2007), source's own date: 1995
- Quotation: "Comprehensive estimates of Ne/N ... averaged only 0·10–0·11." (the tool shortened the sentence with an ellipsis; the rest is paraphrase)
- Read: 2026-11-12
- Confidence: high on the number; the brief's "0.1 to 0.5" upper range was not confirmed by this page (it gave the mean only).
- Use for the ship: wildlife ratio is NOT the right value for a managed colony. Humans in a planned breeding and family-size system can have a much higher ratio; I found no source giving a value for managed humans (see gaps). Plan with 0.25 to 0.5 as an assumption, labelled as the project's own.

### I4. Traill, Bradshaw and Brook 2007: median MVP 4,169
- The number(s): median minimum viable population of 4,169 individuals across 212 species from 141 sources; MVP is context-specific and for most species probably above a few thousand (paraphrase).
- Source: "Minimum viable population size: A meta-analysis of 30 years of published estimates", L.W. Traill, C.J.A. Bradshaw, B.W. Brook, Biological Conservation 139, doi:10.1016/j.biocon.2007.06.011, via Semantic Scholar record https://api.semanticscholar.org/graph/v1/paper/DOI:10.1016/j.biocon.2007.06.011?fields=title,year,abstract, source's own date: 2007 (Crossref: September 2007)
- Quotation: "a median of 4169 individuals" (fragment as returned by the tool)
- Read: 2026-11-12
- Confidence: high on the number; it is for wild species, not humans.
- Use for the ship: a cross-check that a few thousand is a normal order of magnitude for indefinite persistence including random disasters, not only genetics.

### I5. Smith 2014 (Project Hyperion): 14,000 to 44,000, abstract NOT read
- The number(s): the brief gives 14,000 to 44,000. I could NOT confirm it from the paper: Semantic Scholar, OpenAlex and Crossref all hold no abstract for it, ScienceDirect returned 403. A Wikipedia summary (not a source) only says Smith's model gives "in the tens of thousands" and assumed at least one severe catastrophe over a 150-year journey.
- Source: "Estimation of a genetically viable population for multigenerational interstellar voyaging: Review and data for project Hyperion", Cameron M. Smith, Acta Astronautica 97:16-29, doi:10.1016/j.actaastro.2013.12.013, source's own date: 2014 (April)
- Quotation: none (could not read the abstract)
- Read: 2026-11-12 (metadata only)
- Confidence: low for the numbers (the 14,000 to 44,000 comes from the task brief, unverified here); high that the paper exists and what it is about.
- Use for the ship: treat "tens of thousands" as the conservative literature view for 150+ year isolation including catastrophes. It argues for a bank or a bigger population, not for 500 alone.

### I6. Moore 2002: about 160 (or 80 with delayed births), press only
- The number(s): a minimum of 160 people for a 200-year journey with little loss of genetic diversity; 80 if couples in their early twenties delay children until their mid-to-late thirties. The brief's "150 to 180" was not seen.
- Source: John Moore's 2002 JBIS paper was NOT found; the figures reach me only through a Wikipedia summary of a New Scientist news item (Carrington, 15 Feb 2002), which I could not fetch.
- Quotation: none (paraphrase of a secondary summary, no exact quotation)
- Read: 2026-11-12 (secondary summary only)
- Confidence: low (press and a wiki summary only).
- Use for the ship: historical low-end estimate; it is not accepted by the later literature (Smith, Frankham). Do not cite it as settled.

### I7. Gene banks and cryopreserved germplasm: what the sources do and do not say
- The number(s): no source I could read gives a number for how a gene bank changes a minimum population. What the sources state in words: biobanks are "genetic insurance policies"; assisted reproduction is "invaluable" for "genetic management over time"; cryopreservation methods may need review because of developmental plasticity and epigenetic effects.
- Source A: "Biobanking biodiversity: recent strategies, challenges, and opportunities", Lee PC and Comizzoli P, J Reprod Dev 72(3), doi:10.1262/jrd.2025-114, source's own date: 2026 (Europe PMC record).
  Quotation: "These biobanks serve as genetic insurance policies"
- Source B: "Biobanks, offspring fitness and the influence of developmental plasticity in conservation biology", W.V. Holt, Animal Reproduction 20(2):e20230026, doi:10.1590/1984-3143-ar2023-0026, 2023.
  Quotation: "cryopreservation methods themselves may have to be reviewed and revised when planning the biobanks"
- Source C: "Towards Practical Conservation Cloning...", Novak BJ et al., Animals 15(7):989, doi:10.3390/ani15070989, 2025. Cites the black-footed ferret as an applied success of cloning from banked cells (abstract only).
  Quotation: "Recent successes in cloning programs for the endangered black-footed ferret"
- Source D (a gap): the US Fish and Wildlife Service black-footed ferret species page (https://www.fws.gov/species/black-footed-ferret-mustela-nigripes) names "declining genetic fitness including increased inbreeding and a reduction in genetic diversity" as one of four stressors, but its readable text said nothing about frozen sperm, cloning or the founders.
  Quotation: "declining genetic fitness including increased inbreeding and a reduction in genetic diversity"
- Read: 2026-11-12
- Confidence: medium that banking helps in principle (three abstracts agree); low for any quantified benefit.
- Use for the ship: a bank is a re-supply of old variation (it can undo drift, if kept for generations and if the stored sperm, eggs or embryos are numerous and from many unrelated people), but it needs an in-ship fertility lab and a stock. It cannot be counted as extra population in the Ne formulas. Holt 2023 warns the freezing itself may have effects on offspring, so the bank is insurance, not a free substitute.

### I8. Human founder populations: no peer-reviewed numbers found
- Searches by Europe PMC for Tristan da Cunha and Pitcairn returned only unrelated animal papers. No peer-reviewed human figure was read. Not recorded from memory.
- Confidence: n/a. Use for the ship: none; leave this out of the design doc, or research separately.

---
## TOPIC 2: VR and treadmills for confined livestock

### I9. The 2019 Moscow region cow VR trial: press only, ministry release not readable
- The number(s): none verified.
- Source: the Moscow region Ministry of Agriculture's release could not be reached (mcx.mosreg.ru does not resolve; msh.mosreg.ru gave an empty reply). BBC and New Scientist could not be fetched. So I can only say that the trial was reported in the press, and I cannot state its size, duration, claimed mood or milk effects.
- Quotation: none
- Read: 2026-11-12
- Confidence: low (nothing read).
- Use for the ship: describe it as "reported by the press in 2019, with no peer-reviewed results found", nothing more.

### I10. Peer-reviewed VR for farm animals: none found (negative)
- Europe PMC, two queries (VR/virtual environment with cattle/cows/pigs/poultry/livestock and welfare or milk yield; "virtual reality headsets dairy cows milk yield" returned zero records). 303 loose hits on the broader query; the first 8 contained VR used for training students and measuring public opinion, not animals wearing VR. The VR items seen: students touring farms in VR (Anderson et al., J Appl Anim Welf Sci 2026, doi:10.1080/10888705.2025.2559700) and a video-perspective study with students (Ryan et al., Animal Welfare 2026).
- Confidence: medium (one database, not a full literature review).
- Use for the ship: no evidence that VR improves any animal's welfare or milk yield.

### I11. Omnidirectional or VR treadmill for livestock: none found (negative)
- Europe PMC search of treadmill with cattle, cows, pigs, sheep or livestock plus welfare or exercise: six records, none about an omnidirectional or VR treadmill for farm animals. Treadmills appear only as dogs on a standard treadmill (Vet Sci 2026, doi:10.3390/vetsci13090906) and in a mini-review of exercise and myokines in farm animals (Front Vet Sci 2026, doi:10.3389/fvets.2026.1836969). No product or study was found.
- Confidence: medium (one database).
- Use for the ship: treat as nonexistent; a treadmill for a cow would be a design invention with no evidence base.

### I12. What welfare science says confined animals need
- The number(s): none (qualitative).
- Source 1: "The 2020 Five Domains Model: Including Human-Animal Interactions in Assessments of Animal Welfare", Mellor DJ et al., Animals 10(10):1870, doi:10.3390/ani10101870, 2020. Five domains: nutrition, physical environment, health, behavioural interactions (interaction with environment, other animals, humans), mental state.
  Quotation: "2 Physical Environment, 3 Health, 4 Behavioural Interactions and 5 Mental State."
- Source 2: "Some attributes of pasture-based systems as they relate to dairy cow welfare", Fisher MW et al., JDS Communications 7(Suppl 1):63-69, doi:10.3168/jdsc.2025-0947, 2026. Pasture systems give more freedom for natural behaviours and often better health, but have their own risks (feed variability, heat and cold).
  Quotation: "generally having greater freedom for the expression of natural behaviors and often experiencing better health"
- Source 3: Rao/other: "Welfare as a dynamic process ... following relocation to an enriched enclosure" (Animal 2026, doi:10.1016/j.animal.2026.101891): 36 Holstein cows moved to an enriched small-group enclosure, tracked 14 days (cortisol, behaviour, location). Paraphrase only; no result quoted.
- Source 4: Ruiz et al., Poultry Science 2026, doi:10.1016/j.psj.2026.108048: enrichment (almond shells) gave fewer comb-pecking wounds and less fearfulness in hens.
  Quotation: "Environmental enrichment is a promising strategy to improve welfare in laying hens"
- EFSA hen and calf opinions: the Europe PMC title lookup for the EFSA laying-hen opinion returned zero records; not read.
- Read: 2026-11-12
- Confidence: medium (reviews and single studies from abstracts; none a single authoritative checklist).
- Use for the ship: animals need real physical space and surface to move, social contact, enrichment, and ideally outdoor-like access; the ship's livestock decks should be a real space with ground, not a simulated one.

---
## Recommendation (the colony module's population)
My reasoning, from I1 to I7; the Ne/N values for managed humans are the project's assumption, not sourced:
- Without a gene bank, for "indefinitely": aim for Ne at or above 1,000 (I2). With a managed breeding scheme assume Ne/N of 0.25 to 0.5, so N of about 2,000 to 4,000. For safety against catastrophe and an unknown real ratio, plan on 5,000 or more (consistent with the 4,169 wildlife median, I4, and Smith's "tens of thousands" for very long isolation, I5, unconfirmed).
- With a gene bank (frozen sperm, eggs, embryos from many unrelated founders, restocked and replaced): the Ne 100 short-term floor (I2) and strict management make about 500 to 1,000 workable for a few hundred years, and the bank is used to undo drift. I found no number; the claim that a bank lets you halve the size is my design inference, not a finding. The 98 and 160 estimates (I1, I6) are the lower limit with managed breeding and no catastrophes.
- The ship's sector (500): enough for the 5-generation floor (Ne 100 needs Ne/N at least 0.2) only with strict pairing management; NOT enough for "indefinitely" alone. Fine as a unit if sectors exchange people (gene flow), because the real population is the whole drum.
- Drum 1 (about 7,800): clears Ne of 1,000 at a ratio of 0.13 or higher, above the wildlife mean (I3) only after a bit of management, comfortably above a managed 0.25 to 0.5. It is a sound "indefinite" size without a gene bank, if it is one mixing population. Keep a gene bank anyway as insurance (I7).

## Recommendation (VR)
VR is not a credible substitute for space: no peer-reviewed study of it for farm animals was found, no livestock treadmill exists in the literature I searched, and welfare science points to real space, movement and enrichment.

## Could not read
- https://www.sciencedirect.com/robots.txt and /science/article/pii/S0094576513004669 (Smith 2014): HTTP 403.
- https://www.bbc.com/news/world-europe-50566230 and https://www.newscientist.com/article/dn1936-magic-number-for-space-pioneers-calculated/: the fetch tool refused those hosts.
- Moscow region Ministry of Agriculture release: mcx.mosreg.ru did not resolve; msh.mosreg.ru returned nothing.
- Biological Conservation publisher pages (linkinghub.elsevier.com): only a "Redirecting" page.
- en.wikipedia.org/wiki/Virtual_reality_for_cows: 404 (no such page).

## Negative findings and gaps
- No abstract text was obtainable for Smith 2014; the 14,000 to 44,000 figure is unconfirmed.
- Moore 2002 JBIS not found; only secondary report of about 160 (and 80), not 150 to 180.
- Frankham 1995 page gave a mean of 0.10 to 0.11; the "0.1 to 0.5" upper range is not confirmed here.
- No source found with a quantified effect of a gene bank on minimum population size.
- No peer-reviewed human founder-population (Pitcairn, Tristan da Cunha, Amish) figures found with this tool.
- No peer-reviewed VR-for-farm-animals study and no omnidirectional livestock treadmill found (one database).
- The Moscow trial details and the EFSA opinions were not read.

## Summary table
| Question | Recommended number | Findings | Confidence |
|---|---|---|---|
| Short-term genetic floor | Ne at least 100 | I2 | high |
| Long-term (indefinite) genetic target | Ne at least 1,000 | I2 | high |
| Wildlife Ne/N | about 0.10 to 0.11 | I3 | high |
| Managed human Ne/N | assume 0.25 to 0.5 (project assumption, unsourced) | none | low |
| Colony population without a gene bank | 2,000 to 4,000, plan 5,000+ | I2, I3, I4 | medium |
| Colony population with a gene bank | about 500 to 1,000 (inference) | I1, I2, I7 | low |
| Absolute crew floor, managed breeding | 98 | I1 | medium-high |
| Wildlife median MVP | 4,169 | I4 | high |
| Smith 2014 | 14,000 to 44,000 (unconfirmed) | I5 | low |
| Moore 2002 | about 160 (press) | I6 | low |
| Ship sector (500) | unit only, needs gene flow between sectors | I1, I2 | medium |
| Drum 1 (about 7,800) | adequate for indefinite, with a gene bank as insurance | I2, I3, I7 | medium |
| VR for livestock | no peer-reviewed evidence | I10 | medium |
| Omnidirectional treadmill for livestock | none exists in searched literature | I11 | medium |
