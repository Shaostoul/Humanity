# Brief: writing a Library guide

This is the standing brief handed to every agent that writes a document
for the in-app Library. It exists because five content waves re-derived
it from scratch, and because the rules in it were each paid for by a
defect that reached the operator.

Hand it to a writer by pointing at this path. Do not paraphrase it into
a prompt; the wording of the hard rules is load-bearing.

## What you are writing

One document for the in-app Library of HumanityOS, an open-source
platform whose purpose is to teach real life through a simulation. A
reader may be a curious teenager, an adult with no background in the
subject, or somebody who actually needs this to work today. Assume all
three are reading.

## The four layers

A curriculum topic is only a complete experience when it has all four
of: a reading, a skill it levels, simulation data behind it, and a real
cited source. Your document is the reading layer. For the topics handed
out in a wave, the other three usually already exist, so the document
you write is the last missing piece. That is why it is worth doing
slowly.

## House style

Learned across thirty landed guides, not invented here.

- Open on something concrete the reader can picture or do. Never on a
  definition, never on a diagram. "Put a spade into the ground and turn
  it over" is the model.
- Say early what the document will and will not do. Be honest when the
  honest answer is that reading cannot teach the thing. The guide on
  identifying wild plants opens by refusing to be a field guide, and
  that is its best feature.
- Second person. Short paragraphs. Bold lead-ins for the points that
  carry weight.
- Give the number, then say what it means, then say what it does not
  mean. A number with no referent is worse than no number.
- Prefer the specific place. The canonical locale is Silverdale, Kitsap
  County, Washington, on Dyes Inlet, latitude about 47.65 N. When a fact
  is local, say so. When it is general, say that too.
- Cross-reference other guides by relative link, for example
  `[What Soil Is](what_soil_is.md)`. Confirm the filename exists under
  `docs/user/skills/` or `docs/user/making/` before you link it.
- End with a Sources section listing every URL you personally opened,
  what each one gave you, and an explicit note on anything you could not
  source.
- Length: the landed guides run 6,000 to 11,000 words. Do not pad to
  reach that and do not truncate to stay inside it.

## Hard rules

1. **Every factual claim comes from a page you opened this session.**
   In one pass on this repo, 48 of 60 researched species records were
   refuted on adversarial review, almost all of them on toxicity and
   lookalikes. Most recently a record attributed to USDA an odour
   comparison ("musty, similar to parsnip") that appears on neither of
   its cited pages, and used it to justify a conclusion that was itself
   correct. **A correct conclusion reached by an invented reason is the
   defect class here**, because the reader who checks the source finds
   the reason missing and may discard the conclusion with it. If you
   reach a conclusion you cannot source, state the conclusion and say
   plainly that the reason for it is not published.
2. **If a source will not load** (403, paywall, timeout), do not cite it
   and do not paraphrase it from memory. Say so in the Sources section.
   "Uncheckable" is a publishable answer; a guess is not.
3. **ASCII only.** No em dashes, no en dashes, no curly quotes, no
   degree sign, no accented characters, no arrows. A build gate refuses
   them. Write "degrees F", "minus 5", "1 to 3", and plain commas.
4. **Redo every piece of arithmetic you write.** A shipped guide once
   said "December alone (252 mm) exceeds April through September
   combined (294 mm)", which is false on its face and reached the
   operator.
5. **Where a mistake can hurt somebody, say so at the point of the
   mistake**, not in a preamble nobody rereads. Where reading genuinely
   cannot substitute for supervised practice, say that instead of giving
   a rule.
6. **Distinguish established from contested.** Label a rule of thumb as
   a rule of thumb. Say when a cause is not settled in the literature.
7. **Never invent a regulation, a limit, a setback, a dose or a
   temperature.** If the only number you can find is for a different
   jurisdiction or a different scale, say that and do not transplant it.
   One guide refused to state a bluff setback because the only number it
   could open was for a different county, and that refusal was correct.

## Sources you may cite

Read `data/sources/registry.json` first. It lists the authorities with,
for each, whether its words may ship in the bundle (`use: verbatim`, US
federal works, 17 USC 105) or whether we must restate the fact in our
own words (`use: facts`, because facts are not copyrightable per Feist
v. Rural Telephone 1991, but the expression is).

**State agencies are not public domain.** 17 USC 105 covers federal
works only. Restate anything from a state agency.

Roles such as `state-fish-wildlife` and `state-noxious-weeds` resolve
per locale through the `authorities` map in `locale.json`. Cite the role
when the claim is locale-dependent, not the specific agency.

## Local data you can read

    data/locales/silverdale_wa/locale.json     place, authorities map
    data/locales/silverdale_wa/climate.json    normals, extremes
    data/locales/silverdale_wa/soil.json
    data/locales/silverdale_wa/water.json
    data/locales/silverdale_wa/tides.json
    data/locales/silverdale_wa/species.json    122 records
    data/locales/silverdale_wa/phenology.json  68 events, graded
    data/locales/silverdale_wa/hazards.json
    data/locales/silverdale_wa/terrain.json

If your document leans on a record, reference it by its id so the claim
is traceable, and check the id exists.

`phenology.json` grades every event `measured`, `published_range`,
`local_consensus` or `estimated`. Those grades mean different things.
Do not flatten them into "studies show".

**If you find a defect in the shipped data, report it.** Four real
defects have been found exactly this way, by writers who had to read the
data closely enough to teach from it: a tidal range with the wrong name,
an arithmetic claim that was false on its face, a record that disclaimed
a character it had a citation for, and an invented odour comparison. Do
not fix it yourself; report it so the fix gets its own commit and its
own reasoning.

## What you own

Exactly one new file, the path named in your task. Do not edit
`catalog.json`, `syllabus.json`, any existing document, or any data
file. Do not commit. Do not run `build-library.js`. Wiring into the
Library and the curriculum is done by hand afterwards, so that a
document and its catalog row are reviewed together.

## Verify before you report

    node scripts/check-doc-links.js      # must stay at 0 broken
    grep -n '[^ -~\t]' <your file>       # must print nothing
    wc -w <your file>

## What to report

- The word count.
- Every URL you opened and what each one gave you.
- What you could not source, and what you did instead.
- Any defect you found in the shipped data.
- **Any claim in an existing Library guide that your research
  contradicts.** This last one matters more than the document you wrote.
