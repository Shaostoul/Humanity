# How hard an older adult or a child can keep pulling on a rope

**The question:** is there a readable primary source giving the force a person can KEEP UP while
pulling (sustained, as on a hauling line), by age and by sex, so the Calculator's Pulleys mode can
offer lighter pullers (an older adult, a child) beside the Army's soldier?

**Researched: 2026-11-13.** Asked by the operator's answer in `docs/design/pulleys.md` ("Lighter
pullers (an older person, a child) once a primary source is read ... Sure.").

## The short answer

**No, not one that could be read.** The occupational-health pages that could be read give pushing
and pulling figures by SEX only (UK HSE) or for one working population (CCOHS, after Eastman
Kodak), never by age, and both say plainly that their figures are limits for safe work, not what a
person can pull. The only study found that measured children through older adults (the UK DTI's
2000 "Strength Data for Design Safety") measured brief MAXIMUM exertions, not a sustained haul, and
its original report was not found online (only a third-party copy). **So no lighter puller rows
were added**: `data/pulleys/pulleys.ron` keeps its five rows, each citing its source or marked a
design value.

## The findings

### F1. UK Health and Safety Executive: guideline figures by sex, not age, and not limits

- Source: HSE, "Doing a push/pull risk assessment",
  https://www.hse.gov.uk/msd/pushpull/assessment.htm (robots.txt read first: the `/msd/` path is not
  disallowed).
- The source's date: "Updated 2025-03-25".
- Its guideline table (a table, so given as its cells): keeping a load in motion, men "10 kg (about
  100 Newtons)", women "7 kg (about 70 Newtons)"; starting or stopping a load, men "20 kg (about 200
  Newtons)", women "15 kg (about 150 Newtons)".
- The quotation that carries the meaning: "The guideline figures should not be regarded as weight
  limits or approved figures for safe pushing and pulling operations."
- The page says nothing about age, older people, young people or children.

### F2. Canadian Centre for Occupational Health and Safety: one set of upper limits, no sex, no age

- Source: CCOHS, "Pushing and Pulling - General", https://ccohs.ca/oshanswers/ergonomics/push1.html
  (robots.txt read first: `/oshanswers/` is not disallowed).
- The source's dates: "last revised on 2022-12-20"; "Date modified: 2026-10-20".
- Its Table 2 (vertical pulling), "Adopted from: Ergonomic design for people at work. Vol. 2, by
  Eastman Kodak Company, Van Nostrand Reinhold, 1986": pull down above head height 540 N, pull up 25
  cm above the floor 315 N, and lower figures at other heights. Its Table 1 gives combined
  push-and-pull limits (225 N whole body, standing).
- The quotation that carries the meaning: tasks "should be designed for considerably lower force
  requirements than those shown in Table 2". No figure is given by sex or by age.

### F3. The DTI's strength study: children to older adults, but maximum exertions

- What it is: UK Department of Trade and Industry, Government Consumer Safety Research, "Strength
  Data for Design Safety" (2000), about 150 British people from 2 to 90 years old, pushing and
  pulling a bar with one and two hands among other exertions.
- Read only through a search result's summary of a third-party copy (Scribd), not the original, so
  nothing is quoted from it here and nothing is taken from it. As summarised there, it found adults
  (16 to 50) stronger than older adults (51 to 90), who were stronger than children (2 to 10).
- Why it would not settle the question even if read: it measures the most a person can exert for a
  moment, not a pull kept up while hauling, and its authors warn against applying it to other
  postures or comfort-level forces.

## What is still unknown, and what would settle it

- **A sustained hauling force by age and sex.** Settled by a table of maximum acceptable sustained
  pulling forces that gives age bands, from a government or standards body (an ISO 11228-2
  derivation with an age factor, or a national ergonomics handbook), read in full. ISO 11228-2 is a
  priced standard (its price not checked here): the operator's call whether to buy it.
- **The DTI report itself.** Settled by finding the original on the UK Government Web Archive
  (nationalarchives.gov.uk), free; it would give the ratio of older adults' and children's maximum
  pull to adults', which could scale the Army's figure only as a labelled design value, never as a
  measured one.

## Sources we could not reach

- HSE research report RR228, "Review of the risks associated with pushing and pulling heavy
  loads" (https://www.hse.gov.uk/Research/rrhtm/rr228.htm): HSE's robots.txt disallows `/RESEARCH`,
  so it was not read. Hoped for: age effects on pushing and pulling.
- The DTI report's original (F3): not found; only a third-party copy, not read.

## What it means for this project

**Certain:** none of the sources read gives a sustained pulling force by age; the two that give
figures by sex or for a population call them safe-work limits, not capacities, so they measure a
different thing from the Army's "about 100 pounds per man" hauling a line (finding
[`2026-11-13-pulleys-and-rigging.md`](2026-11-13-pulleys-and-rigging.md), F7).

**Judgement call:** adding the HSE's 70 N and 100 N as "a woman" and "a man" beside the soldier's
445 N would mislead (a safe-shift figure for a trolley beside a rope-hauling capacity), so they were
not added. The lighter pullers wait for a source that measures what the row means.
