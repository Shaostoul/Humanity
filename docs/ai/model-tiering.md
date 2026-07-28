# Model tiering: using Sonnet and Opus alongside Fable

Status: ACTIVE PLAN, 2026-07-28 (operator directive: "I don't really want
to waste Fable usage on trivial things that could easily be handled by
Sonnet or Opus... make use of 100% of our usage instead of 100% of Fable
and very little of any of the others." Fable is capped at 50% of total
usage; the other 50% is Opus/Sonnet only and has NEVER been exhausted.)

## The division of labor

The operator's worry is real: smaller models can damage the complex core
(the 17k-line lib.rs, the megashader, the relay, anything cryptographic).
The answer is not "never use them", it is HARD SCOPE WALLS. The tiers:

| Tier | Use for | Never for |
|------|---------|-----------|
| Fable | Renderer/shader work, f64/f32 precision discipline, relay + crypto, cross-file surgery in lib.rs, architecture, incident response, verifying+merging everything below | bulk data entry it can delegate |
| Opus | Feature work in self-contained modules (a new GUI page, a new system in src/systems/), test-writing, web-page mirrors, doc restructuring, review passes over Sonnet output | megashader, relay storage schema, crypto, lib.rs hot regions |
| Sonnet | DATA POPULATION (the big one - see packages below), i18n translations, doc drafting from templates, CSV/RON/JSON authoring against existing schemas, changelog summaries | ANY .rs/.wgsl edit, ANY schema change |

Two mechanics enforce this cheaply:

1. **Session-level**: when Fable's 50% cap hits, keep working in an
   Opus/Sonnet session - but point it ONLY at packages from this file.
   CLAUDE.md discipline (verify before ship, commit early) applies
   unchanged.
2. **Workflow-level (better)**: Fable orchestrates and passes
   `model: 'sonnet'` per agent for bulk stages, keeping
   verification/merge stages on the session model. One Fable session can
   drive dozens of Sonnet data-writers and spot-check their output -
   Fable spends tokens on judgment, Sonnet spends them on volume.

The safety gate for every package below is MECHANICAL, not model trust:
`just validate-data` + `cargo test --features native --lib` + the shipped
-data tests must pass, and a Fable/Opus session reviews the diff before
push. Data files cannot crash the app past those gates (loaders degrade
gracefully by design).

## Sonnet-ready work packages (data volume, schema exists)

1. **The full dictionary** (operator: "a fully fledged dictionary for all
   words, not just our use case, that way anyone can learn any word").
   - Source: English Wiktionary via the kaikki.org machine-readable
     extract (CC BY-SA, attribution in the Library page) or WordNet
     (~155k senses, permissive license) as the conservative start.
   - Architecture (Fable, one increment): `data/dictionary/` sharded by
     first letter (a.json .. z.json), lazy-loaded by the Library
     Dictionary tab on first search of that letter; the current 201-term
     glossary.json stays the curated HumanityOS layer that WINS on
     collision. Native first, web mirrors.
   - Population (Sonnet, many sessions): the conversion script, shard
     hygiene (strip markup, keep sense + part-of-speech + example),
     spot-fix batches flagged by the validator.
2. **Real plants** (operator: "populating the plant list with real plants
   and all their data"). Schema exists (data/plants*, crop_nutrition).
   Sources: USDA PLANTS + OpenFarm (both open data). Sonnet fills rows:
   growth days, spacing, sun, water, hardiness zones, nutrition,
   companions. The farming system reads whatever validates.
3. **i18n completion**: data/i18n has 5 languages with gaps; every new
   page added keys. Sonnet translates key-by-key with the glossary as
   its terminology anchor.
4. **HumanityOS glossary curation**: grow the 201 curated terms to cover
   every label in PAGES.md (the Alt+hover promise).
5. **Tools catalog** (data/tools, 37 entries): expand toward "every
   open-source tool a homesteader/maker needs", with license + platform
   fields validated.
6. **Chemistry depth** (data/chemistry, 396 entries): fill missing
   properties (densities, melting points, toxicity notes) from public
   reference data.
7. **Docs**: keep user/admin/contributor guides in sync with FEATURES.md;
   draft page-level help text (the inline-explanations direction).

## What stays Fable even when it is "just data"

- Anything under data/blueprints/ that lib.rs geometry consumes
  (ship_structure.ron taught us splice corruption is easy).
- schemas/ changes (a schema change is an API change).
- data/coordination/ (the orchestrator journal is load-bearing).

## Bootstrapping a lesser-model session

Point it at this file plus the package name. Standing prompt skeleton:
"You are working on HumanityOS package N from docs/ai/model-tiering.md.
Only touch the files that package names. Run `just validate-data` and the
named tests before claiming done. Do not edit .rs, .wgsl, schemas/, or
anything in CLAUDE.md's hot-file list. Leave a summary in
data/coordination/sessions/." Fable reviews the branch before merge.
